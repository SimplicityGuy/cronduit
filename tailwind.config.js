/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["templates/**/*.html"],
  theme: {
    extend: {
      fontFamily: {
        mono: ["'JetBrains Mono'", "'Fira Code'", "'Source Code Pro'", "'Cascadia Code'", "'Consolas'", "monospace"],
      },
    },
  },
  plugins: [],
};
