// Phase 19 — Cronduit webhook receiver reference (Go, stdlib only).
//
// Listens on 127.0.0.1:9992 and verifies Standard Webhooks v1 signatures
// using HMAC-SHA256 + constant-time compare (`hmac.Equal`). Mirrors the
// form factor of `examples/webhook_mock_server.rs` (Phase 18) but upgrades
// the always-200 mock into a graded-status verifier per the D-12 retry
// contract.
//
// USE ONLY for local maintainer UAT validation. Loopback-bound (127.0.0.1).
// Never expose to the public internet. Production receivers should:
//   - run behind a reverse proxy / TLS terminator
//   - implement working idempotency dedup (this script ships a comment
//     block only — see success branch)
//
// Run modes:
//   go run receiver.go                          HTTP server mode (default)
//   go run receiver.go --verify-fixture <dir>   CI/UAT fixture-verify mode
//
// Cronduit v1.2 ships SHA-256 only. No algorithm-agility (SHA-384/512/Ed25519).
package main

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"
)

// --- Constants -------------------------------------------------------------
const (
	PORT                        = 9992
	LOG_PATH                    = "/tmp/cronduit-webhook-receiver-go.log"
	MAX_BODY_BYTES              = 1 << 20 // 1 MiB; matches webhook_mock_server.rs cap
	MAX_TIMESTAMP_DRIFT_SECONDS = 300     // Standard Webhooks v1 default (D-11)
)

// --- Verify core (copy-pasteable) ------------------------------------------
//
// Constant-time HMAC-SHA256 verify per Standard Webhooks v1 / WH-04.
// Signing string is `${webhook-id}.${webhook-timestamp}.${body}` over the
// BYTE-EXACT body (NEVER json.Unmarshal + json.Marshal — Pitfall 5).
//
//	secret — raw key bytes from os.ReadFile; do NOT trim (Pitfall 3)
//	h      — http.Header (case-insensitive accessor via h.Get)
//	body   — bytes received on the wire
func verifySignature(secret []byte, h http.Header, body []byte) bool {
	return verifyWithDrift(secret, h, body, true)
}

func verifyWithDrift(secret []byte, h http.Header, body []byte, checkDrift bool) bool {
	wid := h.Get("webhook-id")
	wts := h.Get("webhook-timestamp")
	wsig := h.Get("webhook-signature")
	if wid == "" || wts == "" || wsig == "" {
		return false
	}
	ts, err := strconv.ParseInt(wts, 10, 64)
	if err != nil {
		return false
	}
	if checkDrift {
		delta := time.Now().Unix() - ts
		if delta < 0 {
			delta = -delta
		}
		if delta > MAX_TIMESTAMP_DRIFT_SECONDS {
			return false
		}
	}
	mac := hmac.New(sha256.New, secret)
	mac.Write([]byte(wid + "." + wts + "."))
	mac.Write(body)
	expected := mac.Sum(nil)
	// Multi-token parse per Standard Webhooks v1 (forward-compat with v1.3+).
	for _, tok := range strings.Fields(wsig) {
		if !strings.HasPrefix(tok, "v1,") {
			continue
		}
		received, err := base64.StdEncoding.DecodeString(tok[3:])
		if err != nil {
			continue
		}
		// constant-time compare per WH-04
		if hmac.Equal(expected, received) {
			return true
		}
	}
	return false
}

// --- Logging ---------------------------------------------------------------
func logLine(format string, args ...any) {
	line := fmt.Sprintf(format, args...)
	log.Println(line)
	if f, err := os.OpenFile(LOG_PATH, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644); err == nil {
		defer f.Close()
		fmt.Fprintln(f, line)
	}
}

// --- HTTP handler ----------------------------------------------------------
func handler(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Connection", "close")
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	defer func() {
		if rec := recover(); rec != nil {
			logLine("[go-receiver] panic: %v; rejecting 503", rec)
			http.Error(w, "unexpected exception", http.StatusServiceUnavailable)
		}
	}()

	// loopback-only — secret presence checked after body read; justified per D-09 receiver responsibility model
	// 1. Body cap before read (1 MiB).
	body, err := io.ReadAll(http.MaxBytesReader(w, r.Body, MAX_BODY_BYTES))
	if err != nil {
		logLine("[go-receiver] body read error: %v; rejecting 400", err)
		http.Error(w, "body too large or unreadable", http.StatusBadRequest)
		return
	}

	// 2. Read secret from env-var path.
	secretPath := os.Getenv("WEBHOOK_SECRET_FILE")
	if secretPath == "" {
		logLine("[go-receiver] WEBHOOK_SECRET_FILE not set; rejecting 503")
		http.Error(w, "server misconfigured", http.StatusServiceUnavailable)
		return
	}
	secret, err := os.ReadFile(secretPath)
	if err != nil {
		logLine("[go-receiver] cannot read secret: %v; rejecting 503", err)
		http.Error(w, "server misconfigured", http.StatusServiceUnavailable)
		return
	}

	// 3. Map verify outcome to status per D-12 retry contract.
	wid := r.Header.Get("webhook-id")
	wts := r.Header.Get("webhook-timestamp")
	wsig := r.Header.Get("webhook-signature")
	if wid == "" || wts == "" || wsig == "" {
		http.Error(w, "missing required headers", http.StatusBadRequest)
		return
	}
	ts, err := strconv.ParseInt(wts, 10, 64)
	if err != nil {
		http.Error(w, "malformed webhook-timestamp", http.StatusBadRequest)
		return
	}
	delta := time.Now().Unix() - ts
	if delta < 0 {
		delta = -delta
	}
	if delta > MAX_TIMESTAMP_DRIFT_SECONDS {
		http.Error(w, "timestamp drift > 5min", http.StatusBadRequest)
		return
	}
	if !verifySignature(secret, r.Header, body) {
		http.Error(w, "hmac verify failed", http.StatusUnauthorized)
		return
	}

	// 4. Verify success — log + 200.
	// In production: dedupe by webhook-id to handle Phase 20 retries.
	// E.g., short-TTL Set/Map (in-memory) or DB unique constraint on webhook-id.
	// Cronduit may redeliver on transient receiver failures (5xx response → retry t=30s, t=300s).
	// First successful 2xx terminates the retry chain.
	logLine("[go-receiver] verified webhook-id=%s bytes=%d", wid, len(body))
	w.WriteHeader(http.StatusOK)
	fmt.Fprint(w, "OK")
}

// --- Fixture-verify mode ---------------------------------------------------
func verifyFixtureMode(fixtureDir string) int {
	readFile := func(name string) ([]byte, error) {
		return os.ReadFile(fixtureDir + "/" + name)
	}
	secret, err := readFile("secret.txt")
	if err != nil {
		fmt.Fprintf(os.Stderr, "FAIL: cannot read fixture: %v\n", err)
		return 1
	}
	widB, err := readFile("webhook-id.txt")
	if err != nil {
		fmt.Fprintf(os.Stderr, "FAIL: cannot read fixture: %v\n", err)
		return 1
	}
	wtsB, err := readFile("webhook-timestamp.txt")
	if err != nil {
		fmt.Fprintf(os.Stderr, "FAIL: cannot read fixture: %v\n", err)
		return 1
	}
	body, err := readFile("payload.json")
	if err != nil {
		fmt.Fprintf(os.Stderr, "FAIL: cannot read fixture: %v\n", err)
		return 1
	}
	wsigB, err := readFile("expected-signature.txt")
	if err != nil {
		fmt.Fprintf(os.Stderr, "FAIL: cannot read fixture: %v\n", err)
		return 1
	}
	// http.Header is map[string][]string; Get does case-insensitive canonicalize.
	h := http.Header{}
	h.Set("webhook-id", string(widB))
	h.Set("webhook-timestamp", string(wtsB))
	h.Set("webhook-signature", string(wsigB))
	if verifyWithDrift(secret, h, body, false) {
		fmt.Println("OK: fixture verified")
		return 0
	}
	fmt.Fprintln(os.Stderr, "FAIL: fixture did NOT verify")
	return 1
}

func main() {
	if len(os.Args) >= 3 && os.Args[1] == "--verify-fixture" {
		os.Exit(verifyFixtureMode(os.Args[2]))
	}
	addr := fmt.Sprintf("127.0.0.1:%d", PORT)
	logLine("[go-receiver] listening on http://%s/  (log: %s)", addr, LOG_PATH)
	mux := http.NewServeMux()
	mux.HandleFunc("/", handler)
	// Cleartext HTTP is intentional: loopback-bound (127.0.0.1:9992) reference
	// receiver for local maintainer UAT only — see header docstring's
	// "USE ONLY for local maintainer UAT validation" warning. Production
	// receivers run behind a reverse proxy / TLS terminator.
	srv := &http.Server{
		Addr:              addr,
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
	}
	if err := srv.ListenAndServe(); err != nil {
		log.Fatalf("[go-receiver] server error: %v", err)
	}
}
