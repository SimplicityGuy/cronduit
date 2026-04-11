(function(){
  var t = localStorage.getItem('cronduit-theme');
  if (t) document.documentElement.setAttribute('data-theme', t);
  window.__cdToggleTheme = function() {
    var current = document.documentElement.getAttribute('data-theme');
    var next = current === 'light' ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('cronduit-theme', next);
  };
})();
