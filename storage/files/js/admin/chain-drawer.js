/* Chain-of-custody drawer.
 *
 * `components/chain-drawer.hbs` ships an empty shell on every admin page via
 * layout.hbs, and `GET /admin/api/chain/:id` returns the whole envelope. This
 * is the piece that joins them: click (or keyboard-activate) anything carrying
 * `data-chain-id` and the drawer fills with that id's chain.
 *
 * The id may be a decision id, request id, trace id, or session id — the
 * repository resolves all four to a session. When it is a decision id, that
 * decision's own four-stage policy chain drives the stepper; otherwise the
 * stepper falls back to the session's first denial, or its first decision.
 */
(function (global) {
    'use strict';

    var CHAIN_URL = '/admin/api/chain/';
    var STAGE_ORDER = ['scope', 'secret_scan', 'blocklist', 'rate_limit'];

    /* The stepper's `data-stage` names are the drawer's vocabulary; the audit
     * payload writes the policy's own id. These are the two spellings that
     * differ. */
    var STAGE_ALIASES = {
        scope_check: 'scope',
        tool_blocklist: 'blocklist'
    };

    var drawer = null;
    var lastTrigger = null;
    var currentTraceId = '';

    function $(sel, root) {
        return (root || drawer).querySelector(sel);
    }

    function text(el, value) {
        if (el) el.textContent = (value === null || value === undefined || value === '') ? '—' : String(value);
    }

    function usd(microdollars) {
        return '$' + (Number(microdollars || 0) / 1000000).toFixed(4);
    }

    function localTime(iso) {
        if (!iso) return '—';
        var d = new Date(iso);
        return isNaN(d.getTime()) ? String(iso) : d.toLocaleTimeString();
    }

    function stageKey(policyId) {
        return STAGE_ALIASES[policyId] || policyId;
    }

    function clear(el) {
        while (el && el.firstChild) el.removeChild(el.firstChild);
    }

    function el(tag, cls, content) {
        var node = document.createElement(tag);
        if (cls) node.className = cls;
        if (content !== undefined) node.textContent = content;
        return node;
    }

    /* Pick the decision whose policy chain the stepper should show: the one
     * that was clicked, else the first denial (the interesting one in a demo),
     * else the first decision at all. */
    function pickDecision(envelope, id) {
        var decisions = envelope.decisions || [];
        var i;
        for (i = 0; i < decisions.length; i++) {
            if (decisions[i].id === id) return decisions[i];
        }
        for (i = 0; i < decisions.length; i++) {
            if (decisions[i].decision === 'deny') return decisions[i];
        }
        return decisions[0] || null;
    }

    function renderStepper(decision) {
        var stages = {};
        var chain = (decision && decision.evaluated_rules && decision.evaluated_rules.chain) || [];
        for (var i = 0; i < chain.length; i++) {
            stages[stageKey(chain[i].policy_id)] = chain[i];
        }
        for (var s = 0; s < STAGE_ORDER.length; s++) {
            var name = STAGE_ORDER[s];
            var li = $('[data-stage="' + name + '"]');
            if (!li) continue;
            var stage = stages[name];
            var result = stage ? stage.result : '';
            li.className = 'chain-drawer__stage' +
                (result === 'pass' ? ' chain-drawer__stage--pass' :
                 result === 'fail' ? ' chain-drawer__stage--fail' :
                 result ? ' chain-drawer__stage--skipped' : '');
            text($('.chain-drawer__stage-state', li), result || 'not run');
            var detail = li.querySelector('.chain-drawer__stage-detail');
            if (!detail) {
                detail = el('span', 'chain-drawer__stage-detail');
                li.appendChild(detail);
            }
            text(detail, stage ? stage.detail : '');
        }
    }

    function renderTranscript(envelope, decision) {
        var host = $('[data-chain-transcript]');
        if (!host) return;
        clear(host);
        if (decision) {
            host.appendChild(el('p', '', decision.tool_name + ' → ' + decision.decision +
                ' by ' + decision.policy));
            if (decision.reason) host.appendChild(el('p', '', decision.reason));
        }
        var summary = envelope.summary;
        if (summary && (summary.ai_title || summary.ai_summary)) {
            if (summary.ai_title) host.appendChild(el('p', '', summary.ai_title));
            if (summary.ai_summary) host.appendChild(el('p', '', summary.ai_summary));
        }
        if (!host.firstChild) host.appendChild(el('p', 'chain-drawer__empty', 'No transcript captured.'));
    }

    function renderEvents(events) {
        var host = $('[data-chain-events]');
        if (!host) return;
        clear(host);
        if (!events || !events.length) {
            host.appendChild(el('li', 'chain-drawer__empty', 'No tool calls.'));
            return;
        }
        for (var i = 0; i < events.length; i++) {
            var e = events[i];
            var li = el('li', 'chain-drawer__event');
            li.appendChild(el('span', 'chain-drawer__event-time', localTime(e.created_at)));
            li.appendChild(el('span', 'chain-drawer__event-tool', e.tool_name || '—'));
            li.appendChild(el('span', 'chain-drawer__event-type', e.description || e.event_type));
            host.appendChild(li);
        }
    }

    function renderRequests(requests) {
        var table = $('[data-chain-requests]');
        if (!table) return;
        var body = table.querySelector('tbody');
        if (!body) return;
        clear(body);
        if (!requests || !requests.length) {
            var empty = el('tr');
            var cell = el('td', 'chain-drawer__empty', 'No provider call was made.');
            cell.colSpan = 6;
            empty.appendChild(cell);
            body.appendChild(empty);
            return;
        }
        for (var i = 0; i < requests.length; i++) {
            var r = requests[i];
            var tr = el('tr');
            tr.appendChild(el('td', '', localTime(r.created_at)));
            tr.appendChild(el('td', '', r.model));
            tr.appendChild(el('td', '', r.error_message || r.status));
            tr.appendChild(el('td', '', (r.input_tokens || 0) + ' / ' + (r.output_tokens || 0)));
            tr.appendChild(el('td', '', (r.latency_ms || 0) + 'ms'));
            tr.appendChild(el('td', '', usd(r.cost_microdollars)));
            body.appendChild(tr);
        }
    }

    function renderRaw(envelope) {
        var host = $('[data-chain-raw]');
        if (!host) return;
        clear(host);
        host.className = 'chain-drawer__raw-tree json-tree';
        host.setAttribute('data-collapsed', '1');
        host.setAttribute('data-root-label', 'chain');
        host.removeAttribute('data-json-tree-mounted');
        var tree = global.SystempromptAdmin && global.SystempromptAdmin.jsonTree;
        if (tree) {
            tree.mount(host, envelope);
        } else {
            host.appendChild(el('pre', '', JSON.stringify(envelope, null, 2)));
        }
    }

    function render(envelope, id) {
        var decision = pickDecision(envelope, id);
        currentTraceId = envelope.trace_id || envelope.session_id || '';

        text($('[data-chain-trace-id]'), currentTraceId);

        var totals = envelope.totals || {};
        var pill = $('[data-chain-status]');
        if (pill) {
            var denied = Number(totals.deny_count || 0) > 0;
            pill.className = 'chain-drawer__pill ' +
                (denied ? 'chain-drawer__pill--deny' : 'chain-drawer__pill--allow');
            pill.textContent = denied ? totals.deny_count + ' blocked' : 'all allowed';
        }

        var identity = envelope.identity || {};
        text($('[data-chain-identity]'),
            identity.agent_id ? identity.agent_id + ' · ' + (identity.agent_scope || 'user') : identity.user_id);

        text($('[data-chain-total="decisions"]'), totals.decision_count);
        text($('[data-chain-total="denies"]'), totals.deny_count);
        text($('[data-chain-total="cost"]'), usd(totals.total_cost_microdollars));
        text($('[data-chain-total="tokens"]'),
            (totals.total_input_tokens || 0) + ' / ' + (totals.total_output_tokens || 0));

        renderStepper(decision);
        renderTranscript(envelope, decision);
        renderEvents(envelope.events);
        renderRequests(envelope.requests);
        renderRaw(envelope);
    }

    function showError(message) {
        var host = $('[data-chain-transcript]');
        if (!host) return;
        clear(host);
        host.appendChild(el('p', 'chain-drawer__error', message));
    }

    function close() {
        if (!drawer || drawer.hidden) return;
        drawer.hidden = true;
        document.body.style.removeProperty('overflow');
        if (lastTrigger && lastTrigger.focus) lastTrigger.focus();
        lastTrigger = null;
    }

    function open(id, trigger) {
        if (!drawer || !id) return;
        lastTrigger = trigger || null;
        drawer.hidden = false;
        document.body.style.overflow = 'hidden';
        drawer.focus();

        fetch(CHAIN_URL + encodeURIComponent(id), {
            headers: { Accept: 'application/json' },
            credentials: 'same-origin'
        }).then(function (resp) {
            if (!resp.ok) {
                var unwrap = global.AdminApp && global.AdminApp.errorMessage;
                return (unwrap ? unwrap(resp) : Promise.resolve(resp.statusText))
                    .then(function (msg) { throw new Error(msg); });
            }
            return resp.json();
        }).then(function (envelope) {
            render(envelope, id);
        }).catch(function (err) {
            showError(err.message || 'Could not load the chain of custody.');
            if (global.AdminApp && global.AdminApp.Toast) {
                global.AdminApp.Toast.show(err.message, 'error');
            }
        });
    }

    function triggerFor(target) {
        return target && target.closest ? target.closest('[data-chain-id]') : null;
    }

    function init() {
        drawer = document.getElementById('chain-drawer');
        if (!drawer) return;

        document.addEventListener('click', function (ev) {
            if (ev.target.closest('[data-chain-close]')) {
                close();
                return;
            }
            /* A row may hold links of its own; those win over the drawer. */
            if (ev.target.closest('a, button') && !ev.target.closest('[data-chain-copy]')) return;
            var trigger = triggerFor(ev.target);
            if (trigger) {
                ev.preventDefault();
                open(trigger.getAttribute('data-chain-id'), trigger);
            }
        });

        document.addEventListener('keydown', function (ev) {
            if (ev.key === 'Escape') {
                close();
                return;
            }
            if (ev.key !== 'Enter' && ev.key !== ' ') return;
            var trigger = triggerFor(ev.target);
            if (trigger && trigger === ev.target) {
                ev.preventDefault();
                open(trigger.getAttribute('data-chain-id'), trigger);
            }
        });

        var copy = drawer.querySelector('[data-chain-copy]');
        if (copy && navigator.clipboard) {
            copy.addEventListener('click', function () {
                navigator.clipboard.writeText(currentTraceId);
                if (global.AdminApp && global.AdminApp.Toast) {
                    global.AdminApp.Toast.show('Copied ' + currentTraceId, 'success');
                }
            });
        }

        var deepLink = new URLSearchParams(global.location.search).get('chain');
        if (deepLink) open(deepLink, null);
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})(window);
