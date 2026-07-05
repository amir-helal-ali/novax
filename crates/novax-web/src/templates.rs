//! HTML templates for NovaX web UI

/// Base HTML layout — all pages extend this
pub const BASE_LAYOUT: &str = r#"<!DOCTYPE html>
<html lang="ar" dir="rtl">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{{TITLE}}</title>
  <link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><rect width='100' height='100' rx='20' fill='%23c79a3a'/><text x='50' y='70' font-size='60' font-weight='bold' text-anchor='middle' fill='%230f0f10' font-family='sans-serif'>N</text></svg>">
  <link rel="manifest" href="/manifest.json">
  {{SEO_HEAD}}
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    :root {
      --bg: #0f0f10; --bg-card: #1a1a1c; --bg-card-hover: #222225;
      --text: #f0f0f2; --text-muted: #8a8a90;
      --accent: #c79a3a; --accent-bright: #e8b34c;
      --border: #2a2a2d; --green: #518f66; --red: #aa524a; --blue: #6c9bd2;
    }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Naskh Arabic', sans-serif;
      background: var(--bg); color: var(--text); min-height: 100vh; line-height: 1.6;
    }
    .container { max-width: 1200px; margin: 0 auto; padding: 0 24px; }
    a { color: var(--accent); text-decoration: none; }
    a:hover { text-decoration: underline; }

    /* Auth pages */
    .auth-page { display: flex; align-items: center; justify-content: center; min-height: 100vh; padding: 24px; }
    .auth-card { background: var(--bg-card); border: 1px solid var(--border); border-radius: 16px; padding: 40px; max-width: 420px; width: 100%; box-shadow: 0 20px 60px rgba(0,0,0,0.3); }
    .auth-logo { display: flex; align-items: center; gap: 12px; justify-content: center; margin-bottom: 24px; }
    .auth-logo .logo-mark { width: 48px; height: 48px; background: var(--accent); border-radius: 12px; display: flex; align-items: center; justify-content: center; color: var(--bg); font-weight: 800; font-size: 24px; }
    .auth-logo span { font-size: 28px; font-weight: 700; }
    .auth-title { font-size: 24px; font-weight: 600; text-align: center; margin-bottom: 8px; }
    .auth-subtitle { color: var(--text-muted); text-align: center; margin-bottom: 28px; font-size: 14px; }
    .form-group { margin-bottom: 18px; }
    .form-group label { display: block; font-size: 13px; color: var(--text-muted); margin-bottom: 6px; }
    .form-group input, .form-group select, .form-group textarea {
      width: 100%; padding: 12px 14px; background: var(--bg); border: 1px solid var(--border);
      border-radius: 8px; color: var(--text); font-size: 14px; font-family: inherit;
      transition: border-color 0.2s;
    }
    .form-group input:focus, .form-group select:focus, .form-group textarea:focus {
      outline: none; border-color: var(--accent);
    }
    .btn {
      display: inline-flex; align-items: center; justify-content: center; gap: 8px;
      padding: 12px 20px; border-radius: 8px; font-size: 14px; font-weight: 600;
      border: none; cursor: pointer; transition: all 0.2s; font-family: inherit;
      width: 100%;
    }
    .btn-primary { background: var(--accent); color: var(--bg); }
    .btn-primary:hover { background: var(--accent-bright); }
    .btn-secondary { background: transparent; color: var(--text); border: 1px solid var(--border); }
    .btn-secondary:hover { background: var(--bg-card-hover); border-color: var(--accent); }
    .btn-danger { background: var(--red); color: white; }
    .auth-divider { display: flex; align-items: center; gap: 16px; margin: 24px 0; color: var(--text-muted); font-size: 13px; }
    .auth-divider::before, .auth-divider::after { content: ''; flex: 1; height: 1px; background: var(--border); }
    .oauth-buttons { display: flex; flex-direction: column; gap: 10px; }
    .btn-google { background: #fff; color: #333; }
    .btn-google:hover { background: #f5f5f5; }
    .btn-github { background: #24292e; color: #fff; }
    .btn-github:hover { background: #2f363d; }
    .auth-footer { text-align: center; margin-top: 24px; color: var(--text-muted); font-size: 13px; }
    .auth-footer a { color: var(--accent); }
    .alert { padding: 12px 16px; border-radius: 8px; margin-bottom: 18px; font-size: 13px; }
    .alert-error { background: rgba(170,82,74,0.15); border: 1px solid var(--red); color: #ffb4ae; }
    .alert-success { background: rgba(81,143,102,0.15); border: 1px solid var(--green); color: #9fdba8; }
    .alert-info { background: rgba(108,155,210,0.15); border: 1px solid var(--blue); color: #b8d3ed; }

    /* Admin dashboard */
    .admin-header { background: var(--bg-card); border-bottom: 1px solid var(--border); padding: 16px 0; }
    .admin-header-content { display: flex; align-items: center; justify-content: space-between; }
    .admin-header .logo { display: flex; align-items: center; gap: 12px; font-size: 20px; font-weight: 700; }
    .admin-header .logo .logo-mark { width: 36px; height: 36px; background: var(--accent); border-radius: 8px; display: flex; align-items: center; justify-content: center; color: var(--bg); font-weight: 800; font-size: 18px; }
    .admin-nav { display: flex; gap: 8px; align-items: center; }
    .admin-nav a { color: var(--text-muted); padding: 8px 16px; border-radius: 6px; font-size: 14px; transition: all 0.2s; }
    .admin-nav a:hover { background: var(--bg-card-hover); color: var(--text); text-decoration: none; }
    .admin-nav a.active { background: var(--accent); color: var(--bg); }
    .admin-user { display: flex; align-items: center; gap: 10px; color: var(--text-muted); font-size: 13px; }
    .admin-user .avatar { width: 32px; height: 32px; border-radius: 50%; background: var(--accent); display: flex; align-items: center; justify-content: center; color: var(--bg); font-weight: 700; font-size: 13px; }

    .admin-body { display: flex; min-height: calc(100vh - 65px); }
    .admin-sidebar { width: 240px; background: var(--bg-card); border-left: 1px solid var(--border); padding: 24px 0; }
    .admin-sidebar a { display: flex; align-items: center; gap: 12px; padding: 12px 24px; color: var(--text-muted); font-size: 14px; transition: all 0.2s; }
    .admin-sidebar a:hover { background: var(--bg-card-hover); color: var(--text); text-decoration: none; }
    .admin-sidebar a.active { background: rgba(199,154,58,0.1); color: var(--accent); border-right: 3px solid var(--accent); }
    .admin-sidebar .icon { font-size: 18px; width: 24px; text-align: center; }
    .admin-content { flex: 1; padding: 32px; overflow-y: auto; }
    .page-title { font-size: 24px; font-weight: 600; margin-bottom: 8px; }
    .page-subtitle { color: var(--text-muted); margin-bottom: 24px; font-size: 14px; }

    .stats-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 16px; margin-bottom: 32px; }
    .stat-card { background: var(--bg-card); border: 1px solid var(--border); border-radius: 12px; padding: 20px; }
    .stat-label { color: var(--text-muted); font-size: 13px; margin-bottom: 4px; }
    .stat-value { font-size: 28px; font-weight: 700; color: var(--accent-bright); }

    .card { background: var(--bg-card); border: 1px solid var(--border); border-radius: 12px; overflow: hidden; }
    .card-header { padding: 16px 20px; border-bottom: 1px solid var(--border); display: flex; justify-content: space-between; align-items: center; }
    .card-header h3 { font-size: 16px; font-weight: 600; }
    .card-body { padding: 20px; }

    table { width: 100%; border-collapse: collapse; }
    th { text-align: right; padding: 12px 16px; font-size: 12px; color: var(--text-muted); font-weight: 500; text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid var(--border); }
    td { padding: 14px 16px; border-bottom: 1px solid var(--border); font-size: 14px; }
    tr:hover { background: var(--bg-card-hover); }
    td.actions { display: flex; gap: 8px; }
    .badge { display: inline-flex; align-items: center; padding: 4px 10px; border-radius: 12px; font-size: 11px; font-weight: 600; }
    .badge-green { background: rgba(81,143,102,0.2); color: var(--green); }
    .badge-red { background: rgba(170,82,74,0.2); color: var(--red); }
    .badge-blue { background: rgba(108,155,210,0.2); color: var(--blue); }
    .badge-yellow { background: rgba(199,154,58,0.2); color: var(--accent-bright); }

    .pagination { display: flex; gap: 8px; justify-content: center; padding: 20px; }
    .pagination a { padding: 8px 14px; border: 1px solid var(--border); border-radius: 6px; font-size: 13px; color: var(--text); }
    .pagination a:hover { background: var(--bg-card-hover); text-decoration: none; }
    .pagination a.active { background: var(--accent); color: var(--bg); border-color: var(--accent); }

    .form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
    .settings-section { margin-bottom: 32px; }
    .settings-section h3 { font-size: 18px; margin-bottom: 16px; padding-bottom: 12px; border-bottom: 1px solid var(--border); }
    .settings-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
    .settings-item { background: var(--bg-card); border: 1px solid var(--border); border-radius: 8px; padding: 16px; }
    .settings-item label { display: flex; justify-content: space-between; align-items: center; font-size: 14px; }
    .settings-item .desc { color: var(--text-muted); font-size: 12px; margin-top: 6px; }
    .toggle { position: relative; width: 44px; height: 24px; background: var(--border); border-radius: 12px; cursor: pointer; transition: background 0.2s; }
    .toggle.on { background: var(--green); }
    .toggle::after { content: ''; position: absolute; top: 2px; left: 2px; width: 20px; height: 20px; background: white; border-radius: 50%; transition: transform 0.2s; }
    .toggle.on::after { transform: translateX(-20px); }

    @media (max-width: 768px) {
      .admin-sidebar { display: none; }
      .form-row { grid-template-columns: 1fr; }
      .settings-grid { grid-template-columns: 1fr; }
      .stats-grid { grid-template-columns: 1fr 1fr; }
    }
  </style>
</head>
<body>
{{CONTENT}}
</body>
</html>"#;
