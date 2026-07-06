//! صفحات الواجهة الأمامية العامة لـ Novax
//! صفحة الهبوط + لوحة التحكم المحسّنة + بطاقات المشاريع

use novax_seo::{SeoMeta, render_head, website_structured_data, organization_structured_data};

static SEO_CONFIG: once_cell::sync::Lazy<novax_seo::SeoConfig> =
    once_cell::sync::Lazy::new(novax_seo::SeoConfig::default);

/// صفحة الهبوط العامة (عند عدم تسجيل الدخول)
pub fn landing_page() -> String {
    let seo = SeoMeta::new("NovaX — منشئ التطبيقات الموجّه بالنيّة")
        .description("منصة Novax — ابنِ تطبيقات ويب كاملة بلغة Rust بدون كتابة كود. صمّم كياناتك، خصّص مظهرك، صدّر مشروعًا مستقلًا.")
        .canonical("/")
        .keywords(vec![
            "Novax".to_string(), "Rust".to_string(), "Axum".to_string(),
            "HTMX".to_string(), "PostgreSQL".to_string(), "no-code".to_string(),
            "مولّد تطبيقات".to_string(), "Rust web".to_string(),
        ])
        .structured_data(website_structured_data(&SEO_CONFIG))
        .structured_data(organization_structured_data(&SEO_CONFIG));

    let head = render_head(&seo, &SEO_CONFIG);

    format!(r##"<!DOCTYPE html>
<html lang="ar" dir="rtl">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  {head}
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    :root {{
      --bg: #0a0a0b; --bg2: #121214; --card: #1a1a1e; --card-hover: #222228;
      --text: #f0f0f2; --muted: #7a7a82; --accent: #c79a3a; --accent2: #e8b34c;
      --border: #2a2a30; --green: #518f66; --red: #aa524a; --blue: #6c9bd2;
    }}
    body {{ font-family: 'Segoe UI', system-ui, -apple-system, sans-serif; background: var(--bg); color: var(--text); line-height: 1.7; overflow-x: hidden; }}
    a {{ color: var(--accent); text-decoration: none; }}
    a:hover {{ text-decoration: underline; }}

    /* Hero */
    .hero {{ position: relative; min-height: 100vh; display: flex; align-items: center; justify-content: center; text-align: center; padding: 24px; overflow: hidden; }}
    .hero::before {{ content: ''; position: absolute; top: -50%; right: -20%; width: 800px; height: 800px; background: radial-gradient(circle, rgba(199,154,58,0.08) 0%, transparent 70%); pointer-events: none; }}
    .hero::after {{ content: ''; position: absolute; bottom: -30%; left: -10%; width: 600px; height: 600px; background: radial-gradient(circle, rgba(108,155,210,0.06) 0%, transparent 70%); pointer-events: none; }}
    .hero-content {{ position: relative; z-index: 1; max-width: 900px; }}
    .hero-badge {{ display: inline-flex; align-items: center; gap: 8px; padding: 8px 18px; background: rgba(199,154,58,0.1); border: 1px solid rgba(199,154,58,0.3); border-radius: 24px; font-size: 13px; color: var(--accent2); margin-bottom: 32px; }}
    .hero-badge .dot {{ width: 8px; height: 8px; background: var(--green); border-radius: 50%; box-shadow: 0 0 12px var(--green); }}
    .hero h1 {{ font-size: 64px; font-weight: 800; line-height: 1.1; margin-bottom: 24px; background: linear-gradient(135deg, var(--text) 0%, var(--accent2) 100%); -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text; }}
    .hero p {{ font-size: 20px; color: var(--muted); margin-bottom: 40px; max-width: 700px; margin-left: auto; margin-right: auto; }}
    .hero-actions {{ display: flex; gap: 16px; justify-content: center; flex-wrap: wrap; }}
    .btn {{ display: inline-flex; align-items: center; gap: 8px; padding: 14px 32px; border-radius: 12px; font-size: 15px; font-weight: 600; border: none; cursor: pointer; transition: all 0.2s; text-decoration: none; font-family: inherit; }}
    .btn-primary {{ background: var(--accent); color: var(--bg); }}
    .btn-primary:hover {{ background: var(--accent2); transform: translateY(-2px); box-shadow: 0 8px 24px rgba(199,154,58,0.3); text-decoration: none; }}
    .btn-ghost {{ background: transparent; color: var(--text); border: 1px solid var(--border); }}
    .btn-ghost:hover {{ border-color: var(--accent); background: var(--card); text-decoration: none; }}

    /* Features */
    .features {{ padding: 100px 24px; max-width: 1200px; margin: 0 auto; }}
    .section-title {{ text-align: center; font-size: 40px; font-weight: 700; margin-bottom: 16px; }}
    .section-subtitle {{ text-align: center; color: var(--muted); font-size: 18px; margin-bottom: 60px; max-width: 600px; margin-left: auto; margin-right: auto; }}
    .features-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 24px; }}
    .feature-card {{ background: var(--card); border: 1px solid var(--border); border-radius: 16px; padding: 32px; transition: all 0.3s; position: relative; overflow: hidden; }}
    .feature-card:hover {{ border-color: var(--accent); transform: translateY(-4px); box-shadow: 0 12px 40px rgba(0,0,0,0.3); }}
    .feature-card::before {{ content: ''; position: absolute; top: 0; right: 0; width: 100%; height: 3px; background: linear-gradient(90deg, transparent, var(--accent), transparent); opacity: 0; transition: opacity 0.3s; }}
    .feature-card:hover::before {{ opacity: 1; }}
    .feature-icon {{ font-size: 36px; margin-bottom: 16px; }}
    .feature-card h3 {{ font-size: 18px; font-weight: 600; margin-bottom: 8px; }}
    .feature-card p {{ color: var(--muted); font-size: 14px; line-height: 1.6; }}

    /* Workflow */
    .workflow {{ padding: 100px 24px; background: var(--bg2); }}
    .workflow-inner {{ max-width: 1000px; margin: 0 auto; }}
    .steps {{ display: flex; flex-direction: column; gap: 0; }}
    .step {{ display: flex; gap: 24px; padding: 24px 0; border-bottom: 1px solid var(--border); align-items: center; }}
    .step:last-child {{ border-bottom: none; }}
    .step-num {{ flex-shrink: 0; width: 56px; height: 56px; background: var(--accent); color: var(--bg); border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 22px; font-weight: 800; }}
    .step-content h4 {{ font-size: 18px; margin-bottom: 4px; }}
    .step-content p {{ color: var(--muted); font-size: 14px; }}

    /* Stats */
    .stats-bar {{ display: flex; justify-content: center; gap: 60px; padding: 60px 24px; flex-wrap: wrap; }}
    .stat {{ text-align: center; }}
    .stat-num {{ font-size: 48px; font-weight: 800; color: var(--accent2); }}
    .stat-label {{ color: var(--muted); font-size: 14px; margin-top: 4px; }}

    /* CTA */
    .cta {{ padding: 80px 24px; text-align: center; }}
    .cta-box {{ max-width: 700px; margin: 0 auto; background: var(--card); border: 1px solid var(--border); border-radius: 24px; padding: 60px 40px; position: relative; overflow: hidden; }}
    .cta-box::before {{ content: ''; position: absolute; top: -50%; right: -50%; width: 200%; height: 200%; background: radial-gradient(circle, rgba(199,154,58,0.05) 0%, transparent 60%); }}
    .cta-box h2 {{ font-size: 32px; margin-bottom: 16px; position: relative; }}
    .cta-box p {{ color: var(--muted); margin-bottom: 32px; position: relative; }}

    /* Footer */
    .footer {{ padding: 40px 24px; text-align: center; color: var(--muted); font-size: 13px; border-top: 1px solid var(--border); }}
    .footer a {{ color: var(--accent); }}

    @media (max-width: 768px) {{
      .hero h1 {{ font-size: 40px; }}
      .hero p {{ font-size: 16px; }}
      .section-title {{ font-size: 28px; }}
      .step {{ flex-direction: column; text-align: center; }}
      .stats-bar {{ gap: 30px; }}
    }}
  </style>
</head>
<body>

<!-- Hero -->
<section class="hero">
  <div class="hero-content">
    <div class="hero-badge">
      <span class="dot"></span>
      <span>v0.9.0 — يعمل الآن</span>
    </div>
    <h1>منشئ التطبيقات<br>الموجّه بالنيّة</h1>
    <p>
      صمّم كيانات بياناتك، خصّص مظهر تطبيقك، وصدّر مشروع Rust كامل
      — كل ذلك من واجهة بصرية، بدون كتابة سطر كود واحد.
    </p>
    <div class="hero-actions">
      <a href="/auth/login" class="btn btn-primary">🚀 ابدأ الآن</a>
      <a href="/auth/register" class="btn btn-ghost">إنشاء حساب</a>
    </div>
  </div>
</section>

<!-- Stats -->
<div class="stats-bar">
  <div class="stat"><div class="stat-num">100%</div><div class="stat-label">Rust</div></div>
  <div class="stat"><div class="stat-num">0</div><div class="stat-label">JavaScript مكتوب يدويًا</div></div>
  <div class="stat"><div class="stat-num">17</div><div class="stat-label">وحدة (crate)</div></div>
  <div class="stat"><div class="stat-num">∞</div><div class="stat-label">مشاريع قابلة للتصدير</div></div>
</div>

<!-- Features -->
<section class="features">
  <h2 class="section-title">لماذا Novax؟</h2>
  <p class="section-subtitle">منصة واحدة تجمع التصميم البصري وتوليد الكود والإنتاج</p>
  <div class="features-grid">
    <div class="feature-card">
      <div class="feature-icon">🎨</div>
      <h3>مصمّم بصري</h3>
      <p>أنشئ كيانات وحقول وعلاقات من واجهة بصرية. لا حاجة لكتابة SQL أو نماذج Rust يدويًا.</p>
    </div>
    <div class="feature-card">
      <div class="feature-icon">⚙️</div>
      <h3>توليد كود كامل</h3>
      <p>Backend (Axum + SQLx) + Frontend (HTMX + Askama) + SQL migrations + CSS — كل شيء يُولَّد تلقائيًا.</p>
    </div>
    <div class="feature-card">
      <div class="feature-icon">🔓</div>
      <h3>بدون قيود</h3>
      <p>الكود المُصدَّر مستقل تمامًا عن Novax. حمّل tar.gz وشغّله بأمر <code>cargo run</code> على أي خادم.</p>
    </div>
    <div class="feature-card">
      <div class="feature-icon">🔒</div>
      <h3>آمن افتراضيًا</h3>
      <p>SQLx يمنع SQL injection في وقت الترجمة. CSRF protection. Argon2id لكلمات المرور. UUID لكل شيء.</p>
    </div>
    <div class="feature-card">
      <div class="feature-icon">⚡</div>
      <h3>أداء فائق</h3>
      <p>SSR (Askama) + HTMX = First Contentful Paint شبه فوري. لا React، لا Virtual DOM، لا overhead.</p>
    </div>
    <div class="feature-card">
      <div class="feature-icon">🔍</div>
      <h3>API Inspector</h3>
      <p>Swagger UI تلقائي لكل مشروع. جرب الـ endpoints، اعرض schemas، استورد في Postman.</p>
    </div>
  </div>
</section>

<!-- Workflow -->
<section class="workflow">
  <div class="workflow-inner">
    <h2 class="section-title">كيف يعمل؟</h2>
    <p class="section-subtitle">من الفكرة إلى الإنتاج في 5 خطوات</p>
    <div class="steps">
      <div class="step">
        <div class="step-num">1</div>
        <div class="step-content">
          <h4>أنشئ مشروعًا</h4>
          <p>اختر اسمًا ووصفًا لمشروعك. مثال: "MyStore" — متجر إلكتروني.</p>
        </div>
      </div>
      <div class="step">
        <div class="step-num">2</div>
        <div class="step-content">
          <h4>أضف الكيانات والحقول</h4>
          <p>عرّف كياناتك بصريًا: Product (title, price, description), Category (name, icon)...</p>
        </div>
      </div>
      <div class="step">
        <div class="step-num">3</div>
        <div class="step-content">
          <h4>خصّص المظهر</h4>
          <p>اختر الألوان، الخطوط، الزوايا. شاهد المعاينة الحية فورًا.</p>
        </div>
      </div>
      <div class="step">
        <div class="step-num">4</div>
        <div class="step-content">
          <h4>عاين وافحص</h4>
          <p>شاهد الواجهة الأمامية + افحص الـ API عبر Swagger UI المدمج.</p>
        </div>
      </div>
      <div class="step">
        <div class="step-num">5</div>
        <div class="step-content">
          <h4>صدّر وشغّل</h4>
          <p>حمّل المشروع كـ tar.gz. استخرج. شغّل <code>cargo run --release</code>. انتهى!</p>
        </div>
      </div>
    </div>
  </div>
</section>

<!-- CTA -->
<section class="cta">
  <div class="cta-box">
    <h2>جاهز للبدء؟</h2>
    <p>سجّل الدخول بـ admin@novax.local / admin12345 وابدأ بناء مشروعك الأول.</p>
    <a href="/auth/login" class="btn btn-primary" style="position: relative;">دخول لوحة التحكم →</a>
  </div>
</section>

<!-- Footer -->
<footer class="footer">
  <p>NovaX v0.9.0 — مفتوح المصدر تحت Apache-2.0 / MIT ·
  <a href="https://github.com/amir-helal-ali/novax">GitHub</a> ·
  <a href="/api/health">حالة النظام</a> ·
  مبني بـ ❤️ بـ Rust</p>
</footer>

</body>
</html>"##, head = head)
}
