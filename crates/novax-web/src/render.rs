//! Page renderers for NovaX web UI

use crate::templates::BASE_LAYOUT;
use novax_seo::{SeoConfig, SeoMeta, render_head, website_structured_data, organization_structured_data};

/// Simple template substitution: replaces {{KEY}} with value
fn render(template: &str, vars: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
}

/// Global SEO config (lazy-initialized)
static SEO_CONFIG: once_cell::sync::Lazy<SeoConfig> = once_cell::sync::Lazy::new(SeoConfig::default);

/// Wrap content in the base layout with SEO and return HTML
pub fn page(title: &str, content: &str) -> String {
    page_with_seo(title, content, None)
}

/// Wrap content with custom SEO metadata
pub fn page_with_seo(title: &str, content: &str, seo: Option<SeoMeta>) -> String {
    let meta = seo.unwrap_or_else(|| default_seo_meta(title));
    let seo_head = render_head(&meta, &SEO_CONFIG);
    render(BASE_LAYOUT, &[
        ("TITLE", &format!("{} — {}", meta.title, SEO_CONFIG.site_name)),
        ("SEO_HEAD", &seo_head),
        ("CONTENT", content),
    ])
}

/// Default SEO metadata for a page
fn default_seo_meta(title: &str) -> SeoMeta {
    SeoMeta::new(title)
        .description(&SEO_CONFIG.default_description)
        .structured_data(website_structured_data(&SEO_CONFIG))
        .structured_data(organization_structured_data(&SEO_CONFIG))
}

// ─── Auth Pages ───

/// Login page
pub fn login_page(error: Option<&str>, oauth_enabled: bool) -> String {
    page_with_seo("تسجيل الدخول", &login_content(error, oauth_enabled), Some(
        SeoMeta::new("تسجيل الدخول")
            .description("سجل الدخول إلى حسابك في NovaX")
            .canonical("/auth/login")
            .noindex()
    ))
}

fn login_content(error: Option<&str>, oauth_enabled: bool) -> String {
    let error_html = error
        .map(|e| format!(r#"<div class="alert alert-error">{}</div>"#, e))
        .unwrap_or_default();

    let oauth_html = if oauth_enabled {
        let mut buttons = String::new();
        buttons.push_str(r#"<div class="auth-divider">أو سجل الدخول عبر</div><div class="oauth-buttons">"#);
        // Always show both buttons when OAuth section is shown
        buttons.push_str(r#"<a href="/auth/oauth/google" class="btn btn-google" style="width:100%">CONTINUE WITH GOOGLE</a>"#);
        buttons.push_str(r#"<a href="/auth/oauth/github" class="btn btn-github" style="width:100%">CONTINUE WITH GITHUB</a>"#);
        buttons.push_str("</div>");
        buttons
    } else {
        String::new()
    };

    let content = format!(
        r#"<div class="auth-page">
  <div class="auth-card">
    <div class="auth-logo">
      <div class="logo-mark">N</div><span>NovaX</span>
    </div>
    <h1 class="auth-title">تسجيل الدخول</h1>
    <p class="auth-subtitle">أهلاً بعودتك! سجل الدخول للمتابعة.</p>
    {error_html}
    <form method="POST" action="/auth/login">
      <div class="form-group">
        <label>البريد الإلكتروني</label>
        <input type="email" name="email" required autofocus placeholder="you@example.com">
      </div>
      <div class="form-group">
        <label>كلمة المرور</label>
        <input type="password" name="password" required placeholder="••••••••">
      </div>
      <button type="submit" class="btn btn-primary">تسجيل الدخول</button>
    </form>
    {oauth_html}
    <div class="auth-footer">
      ليس لديك حساب؟ <a href="/auth/register">أنشئ حساباً</a><br>
      <a href="/auth/forgot-password">نسيت كلمة المرور؟</a>
    </div>
  </div>
</div>"#,
        error_html = error_html,
        oauth_html = oauth_html,
    );

    content
}

/// Register page
pub fn register_page(error: Option<&str>, oauth_enabled: bool) -> String {
    let error_html = error
        .map(|e| format!(r#"<div class="alert alert-error">{}</div>"#, e))
        .unwrap_or_default();

    let oauth_html = if oauth_enabled {
        r#"<div class="auth-divider">أو سجل عبر</div>
        <div class="oauth-buttons">
          <a href="/auth/oauth/google" class="btn btn-google">SIGN UP WITH GOOGLE</a>
          <a href="/auth/oauth/github" class="btn btn-github">SIGN UP WITH GITHUB</a>
        </div>"#
    } else {
        ""
    };

    let content = format!(
        r#"<div class="auth-page">
  <div class="auth-card">
    <div class="auth-logo">
      <div class="logo-mark">N</div><span>NovaX</span>
    </div>
    <h1 class="auth-title">إنشاء حساب</h1>
    <p class="auth-subtitle">انضم إلى NovaX في دقيقة واحدة.</p>
    {error_html}
    <form method="POST" action="/auth/register">
      <div class="form-group">
        <label>الاسم</label>
        <input type="text" name="name" required autofocus placeholder="اسمك الكامل">
      </div>
      <div class="form-group">
        <label>البريد الإلكتروني</label>
        <input type="email" name="email" required placeholder="you@example.com">
      </div>
      <div class="form-group">
        <label>كلمة المرور</label>
        <input type="password" name="password" required minlength="8" placeholder="8 أحرف على الأقل">
      </div>
      <button type="submit" class="btn btn-primary">إنشاء الحساب</button>
    </form>
    {oauth_html}
    <div class="auth-footer">
      لديك حساب بالفعل؟ <a href="/auth/login">سجل الدخول</a>
    </div>
  </div>
</div>"#,
        error_html = error_html,
        oauth_html = oauth_html,
    );

    page_with_seo("إنشاء حساب", &content, Some(
        SeoMeta::new("إنشاء حساب")
            .description("أنشئ حساباً مجانياً في NovaX — منصة تطوير ويب بلغة Rust")
            .canonical("/auth/register")
            .keywords(vec!["تسجيل".to_string(), "حساب جديد".to_string(), "NovaX".to_string()])
            .structured_data(website_structured_data(&SEO_CONFIG))
    ))
}

/// Forgot password page
pub fn forgot_password_page(error: Option<&str>, success: bool) -> String {
    let msg_html = if success {
        r#"<div class="alert alert-success">إذا كان البريد مسجلاً، ستصلك رسالة لإعادة التعيين خلال دقائق.</div>"#.to_string()
    } else if let Some(e) = error {
        format!(r#"<div class="alert alert-error">{}</div>"#, e)
    } else {
        String::new()
    };

    let form_html = if success {
        r#"<a href="/auth/login" class="btn btn-secondary">العودة لتسجيل الدخول</a>"#
    } else {
        r#"<form method="POST" action="/auth/forgot-password">
          <div class="form-group">
            <label>البريد الإلكتروني</label>
            <input type="email" name="email" required autofocus placeholder="you@example.com">
          </div>
          <button type="submit" class="btn btn-primary">إرسال رابط الاستعادة</button>
        </form>"#
    };

    let content = format!(
        r#"<div class="auth-page">
  <div class="auth-card">
    <div class="auth-logo">
      <div class="logo-mark">N</div><span>NovaX</span>
    </div>
    <h1 class="auth-title">نسيت كلمة المرور</h1>
    <p class="auth-subtitle">أدخل بريدك وسنرسل لك رابط استعادة كلمة المرور.</p>
    {msg_html}
    {form_html}
    <div class="auth-footer">
      <a href="/auth/login">العودة لتسجيل الدخول</a>
    </div>
  </div>
</div>"#,
        msg_html = msg_html,
        form_html = form_html,
    );

    page_with_seo("نسيت كلمة المرور", &content, Some(
        SeoMeta::new("نسيت كلمة المرور")
            .description("استعادة كلمة المرور الخاصة بحسابك في NovaX")
            .canonical("/auth/forgot-password")
            .noindex()
    ))
}

/// Reset password page
pub fn reset_password_page(token: &str, error: Option<&str>, success: bool) -> String {
    let msg_html = if success {
        r#"<div class="alert alert-success">تم تغيير كلمة المرور بنجاح!</div>"#.to_string()
    } else if let Some(e) = error {
        format!(r#"<div class="alert alert-error">{}</div>"#, e)
    } else {
        String::new()
    };

    let form_html: String = if success {
        r#"<a href="/auth/login" class="btn btn-primary">سجل الدخول الآن</a>"#.to_string()
    } else {
        format!(
            r#"<form method="POST" action="/auth/reset-password">
              <input type="hidden" name="token" value="{token}">
              <div class="form-group">
                <label>كلمة المرور الجديدة</label>
                <input type="password" name="password" required minlength="8" autofocus placeholder="8 أحرف على الأقل">
              </div>
              <button type="submit" class="btn btn-primary">تعيين كلمة المرور</button>
            </form>"#,
            token = token,
        )
    };

    let content = format!(
        r#"<div class="auth-page">
  <div class="auth-card">
    <div class="auth-logo">
      <div class="logo-mark">N</div><span>NovaX</span>
    </div>
    <h1 class="auth-title">استعادة كلمة المرور</h1>
    <p class="auth-subtitle">أدخل كلمة مرورك الجديدة.</p>
    {msg_html}
    {form_html}
  </div>
</div>"#,
        msg_html = msg_html,
        form_html = form_html,
    );

    page_with_seo("استعادة كلمة المرور", &content, Some(
        SeoMeta::new("استعادة كلمة المرور")
            .canonical("/auth/reset-password")
            .noindex()
    ))
}

/// Email verification page
pub fn verify_email_page(success: bool, error: Option<&str>) -> String {
    let content: String = if success {
        r#"<div class="auth-page">
  <div class="auth-card" style="text-align: center;">
    <div class="auth-logo">
      <div class="logo-mark">N</div><span>NovaX</span>
    </div>
    <div style="font-size: 64px; margin: 20px 0;">✓</div>
    <h1 class="auth-title">تم التحقق من بريدك!</h1>
    <p class="auth-subtitle">شكراً لك. يمكنك الآن تسجيل الدخول.</p>
    <a href="/auth/login" class="btn btn-primary">سجل الدخول</a>
  </div>
</div>"#.to_string()
    } else {
        let err = error.map(|e| format!(r#"<div class="alert alert-error">{}</div>"#, e)).unwrap_or_default();
        format!(
            r#"<div class="auth-page">
  <div class="auth-card" style="text-align: center;">
    <div class="auth-logo">
      <div class="logo-mark">N</div><span>NovaX</span>
    </div>
    <div style="font-size: 64px; margin: 20px 0;">⚠</div>
    <h1 class="auth-title">فشل التحقق</h1>
    {err}
    <a href="/auth/login" class="btn btn-secondary">العودة</a>
  </div>
</div>"#,
            err = err,
        )
    };
    page_with_seo("التحقق من البريد", &content, Some(
        SeoMeta::new("التحقق من البريد")
            .canonical("/auth/verify-email")
            .noindex()
    ))
}

/// Email verification notice (after register)
pub fn verification_notice_page(email: &str, dev_token: Option<&str>) -> String {
    let dev_token_html = dev_token.map(|t| {
        format!(
            r#"<div class="alert alert-info" style="margin-top: 16px;">
        <strong>وضع التطوير:</strong> رابط التحقق المباشر:<br>
        <a href="/auth/verify-email?token={}">/auth/verify-email?token={}</a>
      </div>"#,
            t, t,
        )
    }).unwrap_or_default();

    let content = format!(
        r#"<div class="auth-page">
  <div class="auth-card" style="text-align: center;">
    <div class="auth-logo">
      <div class="logo-mark">N</div><span>NovaX</span>
    </div>
    <div style="font-size: 48px; margin: 20px 0;">📧</div>
    <h1 class="auth-title">تحقق من بريدك</h1>
    <p class="auth-subtitle">أرسلنا رابط التحقق إلى <strong>{email}</strong>.<br>اضغط على الرابط لتفعيل حسابك.</p>
    {dev_token_html}
    <div class="auth-footer" style="margin-top: 24px;">
      <a href="/auth/login">سجل الدخول</a>
    </div>
  </div>
</div>"#,
        email = email,
        dev_token_html = dev_token_html,
    );
    page_with_seo("تحقق من بريدك", &content, Some(
        SeoMeta::new("تحقق من بريدك")
            .canonical("/auth/register")
            .noindex()
    ))
}

// ─── Admin Dashboard Pages ───

/// Admin dashboard home (overview)
pub fn admin_dashboard(user_email: &str, user_initial: char, stats: &DashboardStats) -> String {
    let content = format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin" class="active"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout" style="margin-top: auto;"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">لوحة التحكم</h1>
    <p class="page-subtitle">نظرة عامة على منصة NovaX</p>
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-label">إجمالي المستخدمين</div>
        <div class="stat-value">{total_users}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">بريد مُحقَّق</div>
        <div class="stat-value">{verified_users}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">مستخدمون نشطون</div>
        <div class="stat-value">{active_users}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">مسؤولون</div>
        <div class="stat-value">{admin_users}</div>
      </div>
    </div>
    <div class="card">
      <div class="card-header">
        <h3>أحدث المستخدمين</h3>
        <a href="/admin/users">عرض الكل ←</a>
      </div>
      <table>
        <thead><tr><th>الاسم</th><th>البريد</th><th>الحالة</th><th>تاريخ الإنشاء</th></tr></thead>
        <tbody>{recent_users_rows}</tbody>
      </table>
    </div>
  </div>
</div>"#,
        admin_header = admin_header("لوحة التحكم", user_email, user_initial),
        total_users = stats.total_users,
        verified_users = stats.verified_users,
        active_users = stats.active_users,
        admin_users = stats.admin_users,
        recent_users_rows = stats.recent_users_rows,
    );

    page_with_seo("لوحة التحكم", &content, Some(
        SeoMeta::new("لوحة التحكم")
            .canonical("/admin")
            .noindex()
    ))
}

/// Admin users list page
pub fn admin_users_page(
    user_email: &str,
    user_initial: char,
    users_rows: &str,
    current_page: u32,
    total_pages: u32,
) -> String {
    let pagination = if total_pages > 1 {
        let mut html = r#"<div class="pagination">"#.to_string();
        for p in 1..=total_pages {
            let active = if p == current_page { "active" } else { "" };
            html.push_str(&format!(
                r#"<a href="/admin/users?page={p}" class="{active}">{p}</a>"#,
                p = p, active = active
            ));
        }
        html.push_str("</div>");
        html
    } else {
        String::new()
    };

    let content = format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users" class="active"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">المستخدمون</h1>
    <p class="page-subtitle">إدارة حسابات المستخدمين</p>
    <div class="card">
      <table>
        <thead>
          <tr>
            <th>الاسم</th>
            <th>البريد</th>
            <th>الحالة</th>
            <th>الدور</th>
            <th>التحقق</th>
            <th>أُنشئ في</th>
            <th>إجراءات</th>
          </tr>
        </thead>
        <tbody>
          {users_rows}
        </tbody>
      </table>
      {pagination}
    </div>
  </div>
</div>"#,
        admin_header = admin_header("المستخدمون", user_email, user_initial),
        users_rows = users_rows,
        pagination = pagination,
    );

    page_with_seo("المستخدمون", &content, Some(
        SeoMeta::new("المستخدمون")
            .canonical("/admin/users")
            .noindex()
    ))
}

/// Admin settings page
pub fn admin_settings_page(
    user_email: &str,
    user_initial: char,
    rate_limit_enabled: bool,
    rate_limit_max: u32,
    rate_limit_window: u64,
    google_enabled: bool,
    github_enabled: bool,
    success: Option<&str>,
) -> String {
    let success_html = success
        .map(|s| format!(r#"<div class="alert alert-success">{}</div>"#, s))
        .unwrap_or_default();

    let content = format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/settings" class="active"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">الإعدادات</h1>
    <p class="page-subtitle">تهيئة منصة NovaX</p>
    {success_html}
    <form method="POST" action="/admin/settings">
      <div class="settings-section">
        <h3>التحكم في الطلبات (Rate Limiting)</h3>
        <div class="settings-grid">
          <div class="settings-item">
            <label>
              تفعيل Rate Limiting
              <div class="toggle {rl_on}" data-toggle="rate_limit_enabled"></div>
              <input type="hidden" name="rate_limit_enabled" value="{rl_value}">
            </label>
            <div class="desc">منع الإفراط في الطلبات من نفس IP</div>
          </div>
          <div class="settings-item">
            <label>الحد الأقصى للطلبات</label>
            <input type="number" name="rate_limit_max" value="{rl_max}" min="1" max="10000">
            <div class="desc">عدد الطلبات المسموح بها في النافذة الزمنية</div>
          </div>
          <div class="settings-item">
            <label>النافذة الزمنية (ثانية)</label>
            <input type="number" name="rate_limit_window" value="{rl_window}" min="1" max="3600">
            <div class="desc">مدة النافذة الزمنية بالثواني</div>
          </div>
        </div>
      </div>
      <div class="settings-section">
        <h3>المصادقة عبر OAuth</h3>
        <div class="settings-grid">
          <div class="settings-item">
            <label>
              Google OAuth
              <div class="toggle {g_on}" data-toggle="google_enabled"></div>
              <input type="hidden" name="google_enabled" value="{g_value}">
            </label>
            <div class="desc">السماح بتسجيل الدخول عبر Google</div>
          </div>
          <div class="settings-item">
            <label>
              GitHub OAuth
              <div class="toggle {gh_on}" data-toggle="github_enabled"></div>
              <input type="hidden" name="github_enabled" value="{gh_value}">
            </label>
            <div class="desc">السماح بتسجيل الدخول عبر GitHub</div>
          </div>
        </div>
      </div>
      <button type="submit" class="btn btn-primary" style="width: auto; padding: 12px 32px;">حفظ الإعدادات</button>
    </form>
  </div>
</div>
<script>
  document.querySelectorAll('.toggle').forEach(t => {{
    t.addEventListener('click', () => {{
      t.classList.toggle('on');
      const name = t.dataset.toggle;
      const input = document.querySelector(`input[name="${{name}}"]`);
      input.value = t.classList.contains('on') ? 'true' : 'false';
    }});
  }});
</script>"#,
        admin_header = admin_header("الإعدادات", user_email, user_initial),
        success_html = success_html,
        rl_on = if rate_limit_enabled { "on" } else { "" },
        rl_value = if rate_limit_enabled { "true" } else { "false" },
        rl_max = rate_limit_max,
        rl_window = rate_limit_window,
        g_on = if google_enabled { "on" } else { "" },
        g_value = if google_enabled { "true" } else { "false" },
        gh_on = if github_enabled { "on" } else { "" },
        gh_value = if github_enabled { "true" } else { "false" },
    );

    page_with_seo("الإعدادات", &content, Some(
        SeoMeta::new("الإعدادات")
            .canonical("/admin/settings")
            .noindex()
    ))
}

// ─── Helpers ───

/// Admin header (with user info + logout)
fn admin_header(title: &str, user_email: &str, user_initial: char) -> String {
    format!(
        r#"<header class="admin-header">
  <div class="container">
    <div class="admin-header-content">
      <div class="logo">
        <div class="logo-mark">N</div>
        <span>NovaX — {title}</span>
      </div>
      <nav class="admin-nav">
        <a href="/admin">لوحة التحكم</a>
        <a href="/admin/users">المستخدمون</a>
        <a href="/admin/settings">الإعدادات</a>
        <div class="admin-user">
          <div class="avatar">{initial}</div>
          <span>{email}</span>
        </div>
        <a href="/auth/logout" class="btn btn-secondary" style="width: auto; padding: 8px 16px;">خروج</a>
      </nav>
    </div>
  </div>
</header>"#,
        title = title,
        initial = user_initial,
        email = user_email,
    )
}

/// Dashboard stats for the overview page
pub struct DashboardStats {
    pub total_users: i64,
    pub verified_users: i64,
    pub active_users: i64,
    pub admin_users: i64,
    pub recent_users_rows: String,
}

/// Profile page (for regular users — view/edit own profile)
pub fn profile_page(
    user_email: &str,
    user_initial: char,
    user: &novax_auth::AuthUser,
    success_msg: Option<&str>,
) -> String {
    let success_html = success_msg
        .map(|m| format!(r#"<div class="alert alert-success">{}</div>"#, m))
        .unwrap_or_default();

    let verified_badge = if user.email_verified_at.is_some() {
        r#"<span class="badge badge-green">✓ مُحقَّق</span>"#
    } else {
        r#"<span class="badge badge-red">غير مُحقَّق</span>"#
    };

    let avatar_html = if let Some(url) = &user.avatar_url {
        format!(r#"<img src="{}" style="width: 80px; height: 80px; border-radius: 50%; object-fit: cover;">"#, url)
    } else {
        format!(r#"<div style="width: 80px; height: 80px; border-radius: 50%; background: var(--accent); display: flex; align-items: center; justify-content: center; color: var(--bg); font-size: 32px; font-weight: 700;">{}</div>"#, user_initial)
    };

    let content = format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/profile" class="active"><span class="icon">👤</span> ملفي الشخصي</a>
    {admin_link}
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">ملفي الشخصي</h1>
    <p class="page-subtitle">عرض وتحديث بياناتك</p>
    {success_html}
    <div style="display: grid; grid-template-columns: 1fr 2fr; gap: 24px;">
      <div class="card">
        <div class="card-body" style="text-align: center;">
          {avatar_html}
          <h3 style="margin-top: 16px;">{name}</h3>
          <p style="color: var(--text-muted); font-size: 14px;">{email}</p>
          <div style="margin-top: 8px;">{verified_badge}</div>
          {role_badge}
        </div>
      </div>
      <div class="card">
        <div class="card-header"><h3>تعديل البيانات</h3></div>
        <div class="card-body">
          <form method="POST" action="/profile/update" enctype="multipart/form-data">
            <div class="form-group">
              <label>الاسم</label>
              <input type="text" name="name" value="{name}" required>
            </div>
            <div class="form-group">
              <label>البريد الإلكتروني</label>
              <input type="email" value="{email}" disabled style="opacity: 0.6;">
            </div>
            <div class="form-group">
              <label>نبذة عنك</label>
              <textarea name="bio" rows="3" placeholder="اكتب نبذة عنك...">{bio}</textarea>
            </div>
            <div class="form-group">
              <label>الصورة الرمزية</label>
              <input type="file" name="avatar" accept="image/png,image/jpeg,image/gif,image/webp" onchange="document.getElementById('avatarForm').submit()">
              <div class="desc">حد أقصى 2MB — PNG, JPG, GIF, WebP</div>
            </div>
            <button type="submit" class="btn btn-primary" style="width: auto; padding: 12px 32px;">حفظ التغييرات</button>
          </form>
          <form id="avatarForm" method="POST" action="/api/users/avatar" enctype="multipart/form-data" style="display:none;"></form>
        </div>
      </div>
    </div>
    <div class="card" style="margin-top: 24px;">
      <div class="card-header"><h3>الأمان</h3></div>
      <div class="card-body">
        <a href="/auth/change-password" class="btn btn-secondary" style="width: auto;">تغيير كلمة المرور</a>
      </div>
    </div>
  </div>
</div>"#,
        admin_header = admin_header("ملفي الشخصي", user_email, user_initial),
        admin_link = if user.is_admin {
            r#"<a href="/admin"><span class="icon">⚙️</span> لوحة الإدارة</a>"#
        } else {
            ""
        },
        success_html = success_html,
        avatar_html = avatar_html,
        name = user.name,
        email = user.email,
        verified_badge = verified_badge,
        role_badge = if user.is_admin {
            r#"<div style="margin-top: 4px;"><span class="badge badge-yellow">مسؤول</span></div>"#
        } else {
            ""
        },
        bio = user.bio.as_deref().unwrap_or(""),
    );

    page_with_seo("ملفي الشخصي", &content, Some(
        SeoMeta::new("ملفي الشخصي")
            .canonical("/profile")
            .noindex()
    ))
}

/// Admin user edit page
pub fn admin_user_edit_page(
    admin_email: &str,
    admin_initial: char,
    user: &novax_auth::AuthUser,
    success_msg: Option<&str>,
) -> String {
    let success_html = success_msg
        .map(|m| format!(r#"<div class="alert alert-success">{}</div>"#, m))
        .unwrap_or_default();

    let content = format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users" class="active"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">تعديل المستخدم</h1>
    <p class="page-subtitle">{user_email}</p>
    {success_html}
    <div class="card">
      <div class="card-header">
        <h3>بيانات المستخدم</h3>
        <a href="/admin/users">← العودة للقائمة</a>
      </div>
      <div class="card-body">
        <form method="POST" action="/admin/users/{user_id}/update">
          <div class="form-row">
            <div class="form-group">
              <label>الاسم</label>
              <input type="text" name="name" value="{name}" required>
            </div>
            <div class="form-group">
              <label>البريد الإلكتروني</label>
              <input type="email" name="email" value="{email}" required>
            </div>
          </div>
          <div class="form-group">
            <label>نبذة</label>
            <textarea name="bio" rows="3">{bio}</textarea>
          </div>
          <div class="form-group">
            <label>رابط الصورة الرمزية</label>
            <input type="url" name="avatar_url" value="{avatar_url}" placeholder="https://...">
          </div>
          <div class="settings-grid">
            <div class="settings-item">
              <label>
                نشط
                <div class="toggle {active_on}" data-toggle="is_active"></div>
                <input type="hidden" name="is_active" value="{active_value}">
              </label>
            </div>
            <div class="settings-item">
              <label>
                مسؤول
                <div class="toggle {admin_on}" data-toggle="is_admin"></div>
                <input type="hidden" name="is_admin" value="{admin_value}">
              </label>
            </div>
          </div>
          <div style="margin-top: 16px;">
            <button type="submit" class="btn btn-primary" style="width: auto; padding: 12px 32px;">حفظ التغييرات</button>
            <a href="/admin/users" class="btn btn-secondary" style="width: auto; padding: 12px 32px; margin-right: 8px;">إلغاء</a>
          </div>
        </form>
      </div>
    </div>
    <div class="card" style="margin-top: 24px;">
      <div class="card-header"><h3>معلومات إضافية</h3></div>
      <div class="card-body">
        <p><strong>تاريخ الإنشاء:</strong> {created_at}</p>
        <p><strong>آخر تحديث:</strong> {updated_at}</p>
        <p><strong>التحقق من البريد:</strong> {verified}</p>
        <p><strong>معرف المستخدم:</strong> <code>{user_id}</code></p>
      </div>
    </div>
  </div>
</div>
<script>
  document.querySelectorAll('.toggle').forEach(t => {{
    t.addEventListener('click', () => {{
      t.classList.toggle('on');
      const name = t.dataset.toggle;
      const input = document.querySelector(`input[name="${{name}}"]`);
      input.value = t.classList.contains('on') ? 'true' : 'false';
    }});
  }});
</script>"#,
        admin_header = admin_header("تعديل مستخدم", admin_email, admin_initial),
        user_email = user.email,
        success_html = success_html,
        user_id = user.id,
        name = user.name,
        email = user.email,
        bio = user.bio.as_deref().unwrap_or(""),
        avatar_url = user.avatar_url.as_deref().unwrap_or(""),
        active_on = if user.is_active { "on" } else { "" },
        active_value = if user.is_active { "true" } else { "false" },
        admin_on = if user.is_admin { "on" } else { "" },
        admin_value = if user.is_admin { "true" } else { "false" },
        created_at = user.created_at.format("%Y-%m-%d %H:%M"),
        updated_at = user.updated_at.format("%Y-%m-%d %H:%M"),
        verified = if user.email_verified_at.is_some() {
            format!("✓ مُحقَّق في {}", user.email_verified_at.unwrap().format("%Y-%m-%d"))
        } else {
            "غير مُحقَّق".to_string()
        },
    );

    page_with_seo("تعديل مستخدم", &content, Some(
        SeoMeta::new("تعديل مستخدم")
            .canonical("/admin/users")
            .noindex()
    ))
}
