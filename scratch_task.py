import sys

def modify_html():
    with open('src/index.html', 'r', encoding='utf-8') as f:
        html = f.read()

    start_grid = html.find('<div class="content-grid">')
    end_page_settings = html.find('        <section id="page-profile" class="page">')
    
    if start_grid == -1 or end_page_settings == -1:
        print("Markers not found")
        return

    # Extract sections
    def extract_panel(label_str, tag='h2'):
        start = html.find(label_str)
        if start == -1: return ""
        panel_start = html.rfind('<section class="panel', 0, start)
        panel_end = html.find('</section>', panel_start) + 10
        return html[panel_start:panel_end]

    snapshot_panel = extract_panel('<section class="panel snapshot-panel">', tag='') 
    # fix snapshot panel because it doesn't have an h2 with a specific label if we just find the class
    snap_start = html.find('<section class="panel snapshot-panel">')
    snap_end = html.find('</section>', snap_start) + 10
    snapshot_panel = html[snap_start:snap_end]

    diagnostics_panel = extract_panel('<h2 data-i18n="diagnosticsSectionTitle">')
    ignored_panel = extract_panel('<h2 data-i18n="settingsIgnoredPortsTitle">')
    theme_panel = extract_panel('<h2 data-i18n="themeSectionTitle">')
    accent_panel = extract_panel('<h2 data-i18n="accentSectionTitle">')
    lang_panel = extract_panel('<h2 data-i18n="languageSectionTitle">')
    updates_panel = extract_panel('<h2 data-i18n="updatesSectionTitle">')
    
    logs_start = html.find('<section class="panel log-panel">')
    logs_end = html.find('</section>', logs_start) + 10
    logs_panel = html[logs_start:logs_end]
    
    acc_panel = extract_panel('<h2>Аккаунт</h2>')
    
    host_start = html.find('<section class="panel panel-large">')
    host_end = html.find('</section>', host_start) + 10
    host_panel = html[host_start:host_end]
    host_panel = host_panel.replace('<section class="panel panel-large">', '<section class="panel panel-large hidden" id="active-session-panel">')

    server_panel = extract_panel('<span class="eyebrow" data-i18n="publicHostsLabel">Список серверов</span>')

    new_content = f'''          <div class="content-grid">
            <div class="stack-column" style="width: 100%;">
              <div class="server-list-controls" style="display: flex; gap: 12px; margin-bottom: 16px;">
                <input type="text" id="server-search-input" placeholder="Поиск серверов..." style="flex: 1; padding: 12px; border-radius: 8px; border: 1px solid var(--line); background: var(--surface-raised); color: var(--text-base); font-size: 14px;" />
                <button type="button" id="open-filter-modal" class="ghost-button" style="padding: 12px;">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/></svg>
                </button>
              </div>

              {server_panel}
              {host_panel}
            </div>
          </div>
        </section>

        <section id="page-settings" class="page">
          <header class="hero-panel settings-hero">
            <div class="hero-left">
              <h1 data-i18n="settingsTitle">Настройки</h1>
            </div>
            <div class="hero-right settings-top-right">
              <span class="status-label" data-i18n="settingsVersionLabel">Версия</span>
              <strong id="settings-version">0.0.0</strong>
            </div>
          </header>

          <nav class="settings-tabs" style="display: flex; gap: 16px; margin-bottom: 24px; border-bottom: 1px solid var(--line); padding-bottom: 8px;">
            <button class="settings-tab active" data-tab="account">Аккаунт</button>
            <button class="settings-tab" data-tab="interface">Интерфейс</button>
            <button class="settings-tab" data-tab="network">Сеть</button>
            <button class="settings-tab" data-tab="diagnostics">Диагностика</button>
          </nav>

          <div class="settings-layout">
            <div class="settings-tab-content active" id="tab-account">
              {acc_panel}
            </div>
            <div class="settings-tab-content" id="tab-interface">
              {theme_panel}
              {accent_panel}
              {lang_panel}
            </div>
            <div class="settings-tab-content" id="tab-network">
              {snapshot_panel}
              {ignored_panel}
            </div>
            <div class="settings-tab-content" id="tab-diagnostics">
              {updates_panel}
              {logs_panel}
              {diagnostics_panel}
            </div>
          </div>
        </section>

'''
    
    final_html = html[:start_grid] + new_content + html[end_page_settings:]
    
    with open('src/index.html', 'w', encoding='utf-8') as f:
        f.write(final_html)
        
    print("Done editing index.html")

modify_html()
