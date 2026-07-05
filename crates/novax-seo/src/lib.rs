//! NovaX SEO
//!
//! Provides SEO utilities: meta tags, Open Graph, Twitter Cards,
//! JSON-LD structured data, sitemap.xml, robots.txt, and canonical URLs.
//!
//! ## Quick Start
//! ```rust,no_run
//! use novax_seo::{SeoMeta, SeoConfig, render_head};
//!
//! let config = SeoConfig::default();
//! let meta = SeoMeta::new("Dashboard — NovaX")
//!     .description("إدارة حسابات المستخدمين")
//!     .canonical("/admin");
//! let head_html = render_head(&meta, &config);
//! ```

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// SEO configuration (site-wide)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeoConfig {
    pub site_name: String,
    pub site_url: String,
    pub default_description: String,
    pub default_image: String,
    pub twitter_handle: Option<String>,
    pub locale: String,
    pub theme_color: String,
    pub keywords: Vec<String>,
}

impl Default for SeoConfig {
    fn default() -> Self {
        Self {
            site_name: "NovaX".to_string(),
            site_url: std::env::var("APP_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            default_description: "منصة تطوير ويب متكاملة بلغة Rust — مصادقة آمنة، لوحة تحكم احترافية، أداء عالٍ".to_string(),
            default_image: "/static/og-image.png".to_string(),
            twitter_handle: std::env::var("TWITTER_HANDLE").ok(),
            locale: "ar_SA".to_string(),
            theme_color: "#c79a3a".to_string(),
            keywords: vec![
                "NovaX".to_string(),
                "Rust".to_string(),
                "web platform".to_string(),
                "PostgreSQL".to_string(),
                "authentication".to_string(),
            ],
        }
    }
}

/// Per-page SEO metadata
#[derive(Debug, Clone)]
pub struct SeoMeta {
    pub title: String,
    pub description: String,
    pub canonical: String,
    pub og_type: String,
    pub og_image: Option<String>,
    pub keywords: Vec<String>,
    pub noindex: bool,
    pub published_time: Option<DateTime<Utc>>,
    pub modified_time: Option<DateTime<Utc>>,
    pub author: Option<String>,
    pub structured_data: Vec<serde_json::Value>,
}

impl SeoMeta {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: String::new(),
            canonical: String::new(),
            og_type: "website".to_string(),
            og_image: None,
            keywords: Vec::new(),
            noindex: false,
            published_time: None,
            modified_time: None,
            author: None,
            structured_data: Vec::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn canonical(mut self, path: impl Into<String>) -> Self {
        self.canonical = path.into();
        self
    }

    pub fn og_type(mut self, t: impl Into<String>) -> Self {
        self.og_type = t.into();
        self
    }

    pub fn og_image(mut self, url: impl Into<String>) -> Self {
        self.og_image = Some(url.into());
        self
    }

    pub fn keywords(mut self, kw: Vec<String>) -> Self {
        self.keywords = kw;
        self
    }

    pub fn noindex(mut self) -> Self {
        self.noindex = true;
        self
    }

    pub fn author(mut self, a: impl Into<String>) -> Self {
        self.author = Some(a.into());
        self
    }

    pub fn published(mut self, t: DateTime<Utc>) -> Self {
        self.published_time = Some(t);
        self
    }

    pub fn modified(mut self, t: DateTime<Utc>) -> Self {
        self.modified_time = Some(t);
        self
    }

    pub fn structured_data(mut self, data: serde_json::Value) -> Self {
        self.structured_data.push(data);
        self
    }
}

/// Render all SEO head tags as HTML
pub fn render_head(meta: &SeoMeta, config: &SeoConfig) -> String {
    let mut html = String::new();

    // Basic meta
    let title = if meta.title.is_empty() {
        config.site_name.clone()
    } else {
        format!("{} — {}", meta.title, config.site_name)
    };
    html.push_str(&format!(r#"<title>{}</title>\n"#, escape_html(&title)));

    let description = if meta.description.is_empty() {
        &config.default_description
    } else {
        &meta.description
    };
    html.push_str(&format!(r#"<meta name="description" content="{}">\n"#, escape_html(description)));

    // Keywords
    let mut all_keywords = config.keywords.clone();
    all_keywords.extend(meta.keywords.clone());
    if !all_keywords.is_empty() {
        html.push_str(&format!(r#"<meta name="keywords" content="{}">\n"#, all_keywords.join(", ")));
    }

    // Author
    if let Some(author) = &meta.author {
        html.push_str(&format!(r#"<meta name="author" content="{}">\n"#, escape_html(author)));
    }

    // Robots
    if meta.noindex {
        html.push_str(r#"<meta name="robots" content="noindex, nofollow">"#);
        html.push('\n');
    } else {
        html.push_str(r#"<meta name="robots" content="index, follow">"#);
        html.push('\n');
    }

    // Canonical URL
    if !meta.canonical.is_empty() {
        let canonical_url = format!("{}{}", config.site_url.trim_end_matches('/'), meta.canonical);
        html.push_str(&format!(r#"<link rel="canonical" href="{}">\n"#, canonical_url));
    }

    // Open Graph
    let og_url = if meta.canonical.is_empty() {
        config.site_url.clone()
    } else {
        format!("{}{}", config.site_url.trim_end_matches('/'), meta.canonical)
    };
    let og_image = meta.og_image.as_deref().unwrap_or(&config.default_image);
    let og_image_url = if og_image.starts_with("http") {
        og_image.to_string()
    } else {
        format!("{}{}", config.site_url.trim_end_matches('/'), og_image)
    };

    html.push_str(&format!(r#"<meta property="og:title" content="{}">\n"#, escape_html(&title)));
    html.push_str(&format!(r#"<meta property="og:description" content="{}">\n"#, escape_html(description)));
    html.push_str(&format!(r#"<meta property="og:type" content="{}">\n"#, meta.og_type));
    html.push_str(&format!(r#"<meta property="og:url" content="{}">\n"#, og_url));
    html.push_str(&format!(r#"<meta property="og:image" content="{}">\n"#, og_image_url));
    html.push_str(&format!(r#"<meta property="og:site_name" content="{}">\n"#, escape_html(&config.site_name)));
    html.push_str(&format!(r#"<meta property="og:locale" content="{}">\n"#, config.locale));

    // Article-specific OG tags
    if meta.og_type == "article" {
        if let Some(t) = meta.published_time {
            html.push_str(&format!(r#"<meta property="article:published_time" content="{}">\n"#, t.to_rfc3339()));
        }
        if let Some(t) = meta.modified_time {
            html.push_str(&format!(r#"<meta property="article:modified_time" content="{}">\n"#, t.to_rfc3339()));
        }
        if let Some(author) = &meta.author {
            html.push_str(&format!(r#"<meta property="article:author" content="{}">\n"#, escape_html(author)));
        }
    }

    // Twitter Card
    html.push_str(r#"<meta name="twitter:card" content="summary_large_image">"#);
    html.push('\n');
    html.push_str(&format!(r#"<meta name="twitter:title" content="{}">\n"#, escape_html(&title)));
    html.push_str(&format!(r#"<meta name="twitter:description" content="{}">\n"#, escape_html(description)));
    html.push_str(&format!(r#"<meta name="twitter:image" content="{}">\n"#, og_image_url));
    if let Some(handle) = &config.twitter_handle {
        html.push_str(&format!(r#"<meta name="twitter:site" content="{}">\n"#, handle));
        html.push_str(&format!(r#"<meta name="twitter:creator" content="{}">\n"#, handle));
    }

    // Theme color
    html.push_str(&format!(r#"<meta name="theme-color" content="{}">\n"#, config.theme_color));

    // Mobile meta
    html.push_str(r#"<meta name="format-detection" content="telephone=no">"#);
    html.push('\n');
    html.push_str(r#"<meta name="apple-mobile-web-app-capable" content="yes">"#);
    html.push('\n');
    html.push_str(r#"<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">"#);
    html.push('\n');
    html.push_str(&format!(r#"<meta name="apple-mobile-web-app-title" content="{}">\n"#, escape_html(&config.site_name)));

    // JSON-LD structured data
    for data in &meta.structured_data {
        html.push_str(&format!(
            r#"<script type="application/ld+json">{}</script>"#,
            serde_json::to_string(data).unwrap_or_default()
        ));
        html.push('\n');
    }

    html
}

/// Generate WebSite structured data (JSON-LD)
pub fn website_structured_data(config: &SeoConfig) -> serde_json::Value {
    serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": config.site_name,
        "url": config.site_url,
        "description": config.default_description,
        "inLanguage": config.locale,
        "potentialAction": {
            "@type": "SearchAction",
            "target": {
                "@type": "EntryPoint",
                "urlTemplate": format!("{}/search?q={{search_term_string}}", config.site_url.trim_end_matches('/'))
            },
            "query-input": "required name=search_term_string"
        }
    })
}

/// Generate Organization structured data (JSON-LD)
pub fn organization_structured_data(config: &SeoConfig) -> serde_json::Value {
    serde_json::json!({
        "@context": "https://schema.org",
        "@type": "Organization",
        "name": config.site_name,
        "url": config.site_url,
        "description": config.default_description,
        "logo": format!("{}{}", config.site_url.trim_end_matches('/'), "/static/logo.png"),
    })
}

/// Generate BreadcrumbList structured data (JSON-LD)
pub fn breadcrumb_structured_data(items: &[(&str, &str)]) -> serde_json::Value {
    let list: Vec<serde_json::Value> = items.iter().enumerate().map(|(i, (name, url))| {
        serde_json::json!({
            "@type": "ListItem",
            "position": i + 1,
            "name": name,
            "item": url
        })
    }).collect();

    serde_json::json!({
        "@context": "https://schema.org",
        "@type": "BreadcrumbList",
        "itemListElement": list
    })
}

// ─── Sitemap ───

/// Sitemap URL entry
#[derive(Debug, Clone, Serialize)]
pub struct SitemapUrl {
    pub loc: String,
    pub lastmod: Option<String>,
    pub changefreq: Option<String>,
    pub priority: Option<f64>,
}

/// Generate sitemap.xml content
pub fn generate_sitemap(base_url: &str, urls: &[SitemapUrl]) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);

    for url in urls {
        xml.push_str("\n  <url>");
        xml.push_str(&format!("\n    <loc>{}</loc>", url.loc));
        if let Some(lastmod) = &url.lastmod {
            xml.push_str(&format!("\n    <lastmod>{}</lastmod>", lastmod));
        }
        if let Some(changefreq) = &url.changefreq {
            xml.push_str(&format!("\n    <changefreq>{}</changefreq>", changefreq));
        }
        if let Some(priority) = url.priority {
            xml.push_str(&format!("\n    <priority>{:.1}</priority>", priority));
        }
        xml.push_str("\n  </url>");
    }

    xml.push_str("\n</urlset>");
    xml
}

/// Default sitemap for NovaX (public pages only)
pub fn default_sitemap(base_url: &str) -> Vec<SitemapUrl> {
    let now = Utc::now().format("%Y-%m-%d").to_string();
    vec![
        SitemapUrl {
            loc: base_url.to_string(),
            lastmod: Some(now.clone()),
            changefreq: Some("daily".to_string()),
            priority: Some(1.0),
        },
        SitemapUrl {
            loc: format!("{}/auth/login", base_url.trim_end_matches('/')),
            lastmod: Some(now.clone()),
            changefreq: Some("monthly".to_string()),
            priority: Some(0.6),
        },
        SitemapUrl {
            loc: format!("{}/auth/register", base_url.trim_end_matches('/')),
            lastmod: Some(now.clone()),
            changefreq: Some("monthly".to_string()),
            priority: Some(0.6),
        },
        SitemapUrl {
            loc: format!("{}/api/health", base_url.trim_end_matches('/')),
            lastmod: Some(now),
            changefreq: Some("weekly".to_string()),
            priority: Some(0.3),
        },
    ]
}

/// Generate robots.txt content
pub fn generate_robots_txt(base_url: &str) -> String {
    format!(
        r#"User-agent: *
Allow: /
Disallow: /admin
Disallow: /api/users
Disallow: /api/auth
Disallow: /profile

# Sitemap
Sitemap: {}/sitemap.xml

# Crawl-delay (be nice)
Crawl-delay: 1
"#,
        base_url.trim_end_matches('/')
    )
}

/// Generate PWA manifest.json
pub fn generate_manifest(config: &SeoConfig) -> serde_json::Value {
    serde_json::json!({
        "name": config.site_name,
        "short_name": config.site_name,
        "description": config.default_description,
        "start_url": "/",
        "display": "standalone",
        "background_color": "#0f0f10",
        "theme_color": config.theme_color,
        "orientation": "portrait-primary",
        "dir": "rtl",
        "lang": "ar",
        "icons": [
            {
                "src": "/static/icon-192.png",
                "sizes": "192x192",
                "type": "image/png",
                "purpose": "any maskable"
            },
            {
                "src": "/static/icon-512.png",
                "sizes": "512x512",
                "type": "image/png",
                "purpose": "any maskable"
            }
        ]
    })
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_head_includes_og_tags() {
        let config = SeoConfig::default();
        let meta = SeoMeta::new("Test Page").description("Test description");
        let html = render_head(&meta, &config);
        assert!(html.contains("og:title"));
        assert!(html.contains("og:description"));
        assert!(html.contains("twitter:card"));
        assert!(html.contains("theme-color"));
    }

    #[test]
    fn test_sitemap_generation() {
        let urls = default_sitemap("http://localhost:3000");
        let xml = generate_sitemap("http://localhost:3000", &urls);
        assert!(xml.contains("<urlset"));
        assert!(xml.contains("<loc>http://localhost:3000</loc>"));
        assert!(xml.contains("<loc>http://localhost:3000/auth/login</loc>"));
    }

    #[test]
    fn test_robots_txt() {
        let txt = generate_robots_txt("http://localhost:3000");
        assert!(txt.contains("User-agent: *"));
        assert!(txt.contains("Disallow: /admin"));
        assert!(txt.contains("Sitemap: http://localhost:3000/sitemap.xml"));
    }
}
