// Entities pages: size the CSS bar charts from their data attributes and render
// the trace waterfall from its embedded span payload. All server-rendered; this
// only sizes/draws what the SSR templates emit.
(function () {
  'use strict';

  function sizeBars(selector, dataAttr, maxAttr) {
    document.querySelectorAll(selector).forEach(function (container) {
      var max = parseFloat(container.getAttribute(maxAttr)) || 0;
      container.querySelectorAll('[' + dataAttr + ']').forEach(function (bar) {
        var v = parseFloat(bar.getAttribute(dataAttr)) || 0;
        var pct = max > 0 ? Math.max(2, (v / max) * 100) : 0;
        bar.style.height = pct + '%';
      });
    });
  }

  function renderWaterfall() {
    var root = document.querySelector('[data-waterfall]');
    var payload = document.getElementById('trace-spans');
    if (!root || !payload) return;

    var spans;
    try {
      spans = JSON.parse(payload.textContent || '[]');
    } catch (e) {
      return;
    }
    if (!spans.length) {
      root.innerHTML = '<p class="text-tertiary">No spans to plot.</p>';
      return;
    }

    var starts = spans.map(function (s) { return new Date(s.started_at).getTime(); });
    var ends = spans.map(function (s) { return new Date(s.ended_at).getTime(); });
    var t0 = Math.min.apply(null, starts);
    var t1 = Math.max.apply(null, ends);
    var span = Math.max(1, t1 - t0);

    var frag = document.createDocumentFragment();
    spans.forEach(function (s) {
      var start = new Date(s.started_at).getTime();
      var end = new Date(s.ended_at).getTime();
      var left = ((start - t0) / span) * 100;
      var width = Math.max(0.5, ((end - start) / span) * 100);

      var row = document.createElement('div');
      row.className = 'waterfall__row';

      var label = document.createElement('span');
      label.className = 'waterfall__label';
      label.textContent = s.name || s.kind;
      row.appendChild(label);

      var track = document.createElement('div');
      track.className = 'waterfall__track';

      var bar = document.createElement('div');
      bar.className = 'waterfall__bar waterfall__swatch--' + s.kind +
        (s.status === 'deny' || s.status === 'error' ? ' waterfall__bar--bad' : '');
      bar.style.left = left + '%';
      bar.style.width = width + '%';
      bar.title = s.name + ' · ' + s.duration_ms + ' ms · ' + s.status;

      var dur = document.createElement('span');
      dur.className = 'waterfall__dur';
      dur.textContent = s.duration_ms + ' ms';
      bar.appendChild(dur);

      track.appendChild(bar);
      row.appendChild(track);
      frag.appendChild(row);
    });

    root.innerHTML = '';
    root.appendChild(frag);
  }

  function init() {
    sizeBars('.latency-histogram', 'data-count', 'data-histogram-max');
    sizeBars('.cost-spark', 'data-cost', 'data-cost-max');
    renderWaterfall();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
