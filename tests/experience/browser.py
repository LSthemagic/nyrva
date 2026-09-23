"""Browser E2E of the shipped dashboard with an injected, versioned IPC boundary.
This is not native desktop acceptance. No test hooks are shipped in the product.
Run: python tests/experience/browser.py --output experience-evidence/browser
"""
import argparse
import functools
import http.server
import json
from pathlib import Path
import threading
import time
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[2]


def fixture():
    now = int(time.time() * 1000)
    settings = dict(schema_version=1, layout='compact', retention_days=90, collect_session_metadata=True,
                    providers=dict(claude=True, codex=True, cursor=True, antigravity=True),
                    alerts=dict(enabled=True, thresholds=[0.2, 0.1], quiet_until_ms=0), reserve_fraction=0.1,
                    account_aliases={'active': 'Pessoal'})
    quota = dict(schema_version=1, provider='antigravity', account_id='active', source='statusline', status='live', observed_at_ms=now,
                 buckets=[dict(id='weekly', label='<img src=x onerror=alert(1)>', remaining_fraction=0, resets_at_ms=now-1000, count=None, estimated=False)],
                 model=None, context=None, session=None, metrics=None)
    session = dict(provider='claude', account_id='active', source='statusline', status='live', session_status='recent', observed_at_ms=now,
                   model='modelo informado', context=dict(used_percentage=0, size=200000),
                   session=dict(id='session-1', parent_id=None, project='Projeto local', branch='main', state='working'),
                   metrics=dict(estimated_cost_usd=0, duration_ms=2000, effort='high'))
    providers = [dict(id=p, enabled=True, status='live' if p == 'antigravity' else 'unavailable',
                     sources=[dict(source='statusline', account_id='active', status='live', observed_at_ms=now, age_ms=0, bucket_count=1)] if p == 'antigravity' else [],
                     capabilities=dict(quota=p == 'antigravity', sessions=p == 'claude', billing=False)) for p in settings['providers']]
    return dict(schema_version=1, captured_at_ms=now, settings=settings, quotas=[quota], providers=providers, sessions=[session], session_count=1, sessions_truncated=False,
                projects=[dict(project='Projeto local', session_count=1, estimated_cost_usd=0, billed_cost_usd=None, cost_coverage='complete_for_observed_sessions', currency='USD', quota_attribution='unavailable')],
                agents=dict(availability='reported_metadata', inferred_relationships=False, nodes=[dict(id='session-1', provider='claude', account_id='active', source='statusline', parent_id=None, reported_parent_id='missing', relationship='unresolved', model='modelo informado', freshness='recent')]),
                pulse=dict(state='unknown', capacity_percentage=None, providers=[dict(provider=p, state='unknown', lowest_reported_bucket_remaining=None) for p in settings['providers']]),
                resets=[dict(provider='antigravity', account_id='active', source='statusline', bucket_id='weekly', label='Weekly', resets_at_ms=now-1000, awaiting_confirmation=True)],
                alerts=[], alerts_error=None)


def main():
    args = argparse.ArgumentParser()
    args.add_argument('--output', default='experience-evidence/browser')
    output = Path(args.parse_args().output)
    output.mkdir(parents=True, exist_ok=True)
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(ROOT / 'nyrva' / 'ui'))
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    checks = []
    with sync_playwright() as p:
        browser = p.chromium.launch()
        context = browser.new_context(viewport={'width': 1280, 'height': 900}, reduced_motion='reduce')
        context.tracing.start(screenshots=True, snapshots=True, sources=True)
        context.add_init_script('''
          window.fixture = FIXTURE;
          window.calls = []; window.listeners = {}; window.failRead = false;
          const clone = x => JSON.parse(JSON.stringify(x));
          window.__TAURI__ = {
            core: { invoke: async (command, args = {}) => {
              window.calls.push({command, args: clone(args)});
              const action = command.split('|').pop();
              if(action === 'read_cockpit') { if(window.failRead) throw Error('offline'); return clone(window.fixture); }
              if(action === 'save_settings') { window.fixture.settings = clone(args.settings); window.fixture.captured_at_ms = Date.now(); return clone(window.fixture); }
              if(action === 'export_settings') return clone(window.fixture.settings);
              if(action === 'import_settings') { window.fixture.settings = JSON.parse(args.text); return clone(window.fixture); }
              if(action === 'rollback_settings') return clone(window.fixture);
              if(action === 'read_history') return {schema_version:1,samples:clone(window.fixture.quotas),truncated:false,forecasts:[]};
              if(action === 'clear_history') { if(args.confirmation !== 'CLEAR NYRVA') throw Error('confirmation missing'); window.fixture.quotas=[]; window.fixture.sessions=[]; window.fixture.projects=[]; window.fixture.agents.nodes=[]; window.fixture.resets=[]; window.fixture.alerts=[]; window.fixture.captured_at_ms=Date.now(); return clone(window.fixture); }
              throw Error('unexpected IPC: '+command);
            }},
            event: { listen: async (name, callback) => { window.listeners[name] = callback; return () => delete window.listeners[name]; } }
          };
        '''.replace('FIXTURE', json.dumps(fixture())))
        page = context.new_page()
        errors = []
        page.on('pageerror', lambda e: errors.append(str(e)))
        try:
            response = page.goto(f'http://127.0.0.1:{server.server_port}/dashboard.html')
            assert response.status == 200, 'dashboard entrypoint has not been implemented'
            expect(page.locator('#quota-cards')).to_contain_text('0%')
            expect(page.locator('#quota-cards')).to_contain_text('Aguardando confirmação')
            expect(page.locator('#quota-cards')).to_contain_text('<img src=x onerror=alert(1)>')
            assert page.locator('#quota-cards img').count() == 0
            checks.append('zero, expired reset and untrusted metadata')
            page.screenshot(path=str(output / 'overview.png'), full_page=True)
            page.get_by_role('tab', name='Sessões', exact=True).click()
            expect(page.locator('#panel-sessions')).to_contain_text('Projeto local')
            expect(page.locator('#panel-sessions')).to_contain_text('0%')
            page.get_by_role('tab', name='Projetos e agentes', exact=True).click()
            expect(page.locator('#panel-projects')).to_contain_text('não resolvido')
            expect(page.locator('#panel-projects')).to_contain_text('Estimativa')
            checks.append('reported sessions, context, estimated cost and unresolved agents')
            page.get_by_role('tab', name='Histórico', exact=True).click()
            page.get_by_role('button', name='Consultar histórico', exact=True).click()
            expect(page.locator('#history-results')).to_contain_text('0%')
            checks.append('history query and accessible measurement table')
            page.get_by_role('tab', name='Fontes', exact=True).click()
            expect(page.locator('#panel-sources')).to_contain_text('statusline')
            page.get_by_role('tab', name='Privacidade e ajustes', exact=True).click()
            page.get_by_label('Layout do painel', exact=True).select_option('bars')
            page.get_by_role('button', name='Salvar preferências', exact=True).click()
            expect(page.locator('body')).to_have_attribute('data-layout', 'bars')
            page.get_by_label('Layout do painel', exact=True).select_option('reset-first')
            page.get_by_role('button', name='Salvar preferências', exact=True).click()
            expect(page.locator('body')).to_have_attribute('data-layout', 'reset-first')
            assert page.evaluate("calls.filter(c=>c.command.endsWith('|save_settings')).length") == 2
            checks.append('all layouts persist through validated IPC')
            page.get_by_label('Retenção em dias', exact=True).fill('30')
            page.evaluate("listeners.observatory({payload:{...fixture,captured_at_ms:Date.now()}})")
            expect(page.get_by_label('Retenção em dias', exact=True)).to_have_value('30')
            checks.append('live refresh does not overwrite unsaved preferences')
            page.get_by_role('button', name='Silenciar por 1 hora', exact=True).click()
            assert page.evaluate('fixture.settings.alerts.quiet_until_ms > Date.now()')
            checks.append('alert silence is persisted')
            page.screenshot(path=str(output / 'privacy.png'), full_page=True)
            page.get_by_role('tab', name='Visão geral', exact=True).click()
            page.evaluate('window.failRead=true')
            page.get_by_role('button', name='Atualizar painel', exact=True).click()
            expect(page.locator('#connection')).to_contain_text('última leitura')
            expect(page.locator('#quota-cards')).to_contain_text('0%')
            page.evaluate('window.failRead=false')
            checks.append('read failures keep last known values and warn')
            page.get_by_role('tab', name='Visão geral', exact=True).focus()
            page.keyboard.press('ArrowRight')
            expect(page.get_by_role('tab', name='Sessões', exact=True)).to_be_focused()
            page.keyboard.press('End')
            expect(page.get_by_role('tab', name='Privacidade e ajustes', exact=True)).to_be_focused()
            checks.append('keyboard tabs and focus')
            page.get_by_role('tab', name='Privacidade e ajustes', exact=True).click()
            page.get_by_role('button', name='Limpar histórico do Nyrva', exact=True).click()
            assert page.evaluate("calls.filter(c=>c.command.endsWith('|clear_history')).length") == 0
            page.get_by_role('button', name='Cancelar', exact=True).click()
            page.get_by_role('button', name='Limpar histórico do Nyrva', exact=True).click()
            page.get_by_role('button', name='Confirmar limpeza', exact=True).click()
            expect(page.get_by_role('dialog')).not_to_be_visible()
            assert page.evaluate("calls.filter(c=>c.command.endsWith('|clear_history')).length") == 1
            checks.append('explicit privacy confirmation and cancellation')
            page.get_by_role('tab', name='Visão geral', exact=True).click()
            expect(page.locator('#quota-cards')).to_contain_text('Nenhuma quota')
            page.set_viewport_size({'width': 640, 'height': 800})
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'horizontal overflow'
            assert not errors, errors
            checks.append('empty state, narrow viewport and no JavaScript exceptions')
            (output / 'result.json').write_text(json.dumps({'passed': checks, 'native_acceptance': False}, indent=2), encoding='utf-8')
            print(json.dumps({'passed': checks, 'native_acceptance': False}))
        except Exception:
            page.screenshot(path=str(output / 'failure.png'), full_page=True)
            (output / 'partial.json').write_text(json.dumps({'passed': checks, 'errors': errors}, indent=2), encoding='utf-8')
            raise
        finally:
            context.tracing.stop(path=str(output / 'trace.zip'))
            browser.close()
            server.shutdown()

if __name__ == '__main__':
    main()
