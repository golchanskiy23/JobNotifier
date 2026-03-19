use async_trait::async_trait;
use chromiumoxide::Browser;
use chromiumoxide::browser::BrowserConfig;
use futures::StreamExt;
use chrono::Utc;

use crate::domain::{Job, Url};
use crate::errors::ScraperError;
use crate::scraper::Scraper;
use crate::scraper::universal::UniversalScraper;
use crate::scraper::grade::detect_grade;

pub struct BrowserScraper {
    inner: UniversalScraper,
    chrome_path: Option<String>,
    wait_ms: u64,
}

impl BrowserScraper {
    pub fn new(
        keywords: Vec<String>,
        user_agent: Option<String>,
        chrome_path: Option<String>,
        wait_ms: Option<u64>,
    ) -> Self {
        Self {
            inner: UniversalScraper::new(keywords, user_agent),
            chrome_path,
            wait_ms: wait_ms.unwrap_or(3000),
        }
    }

    fn jobs_from_js_results(&self, json_str: &str, base_url: &str) -> Result<Vec<Job>, ScraperError> {
        let base_host = UniversalScraper::extract_host_pub(base_url);
        let items: Vec<serde_json::Value> = serde_json::from_str(json_str).unwrap_or_default();
        let mut jobs = Vec::new();
        for item in items {
            let title = item["title"].as_str().unwrap_or("").trim().to_string();
            let url = item["url"].as_str().unwrap_or("").to_string();
            if title.is_empty() || url.is_empty() { continue; }
            let url_host = UniversalScraper::extract_host_pub(&url);
            // Принимаем ссылки на тот же домен ИЛИ относительные пути (начинаются с /)
            let same_domain = base_host.is_empty()
                || url_host.is_empty()
                || url_host == base_host
                || url.starts_with('/');
            if !same_domain { continue; }
            // Исключаем нерелевантные разделы
            let url_lower = url.to_lowercase();
            if url_lower.contains("/event/") || url_lower.contains("/events/")
                || url_lower.contains("/blog/") || url_lower.contains("/news/")
                || url_lower.contains("/article/") || url_lower.contains("/press/") {
                continue;
            }
            // Разрешаем относительные URL — делаем абсолютными
            let abs_url = if url.starts_with('/') {
                let origin = if let Some(pos) = base_url.find("://") {
                    let after = &base_url[pos+3..];
                    let end = after.find('/').unwrap_or(after.len());
                    &base_url[..pos+3+end]
                } else { base_url };
                format!("{}{}", origin, url)
            } else {
                url
            };
            let grade = detect_grade(&title);
            jobs.push(Job {
                id: format!("{}-{}", Utc::now().timestamp_nanos_opt().unwrap_or(0), title.chars().take(10).collect::<String>()),
                title,
                company: String::new(),
                tech_stack: vec![],
                grade,
                url: Url(abs_url),
                salary: None,
                seen_at: Utc::now(),
            });
        }
        Ok(jobs)
    }

    async fn fetch_jobs(&self, url: &str) -> Result<Vec<Job>, ScraperError> {
        let mut config_builder = BrowserConfig::builder()
            .arg("--no-sandbox")
            .arg("--disable-setuid-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-gpu")
            // Маскируем headless — убираем признаки автоматизации
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-infobars")
            .arg("--window-size=1920,1080")
            .arg("--start-maximized");

        if let Some(ref path) = self.chrome_path {
            config_builder = config_builder.chrome_executable(path);
        }

        let config = config_builder.build().map_err(|e| ScraperError::Browser {
            url: url.to_string(),
            message: format!("browser config error: {}", e),
        })?;

        let (mut browser, mut handler) = Browser::launch(config).await.map_err(|e| ScraperError::Browser {
            url: url.to_string(),
            message: format!("browser launch error: {}", e),
        })?;

        let handler_task = tokio::task::spawn(async move {
            loop { if handler.next().await.is_none() { break; } }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let page = browser.new_page("about:blank").await.map_err(|e| ScraperError::Browser {
            url: url.to_string(),
            message: format!("new page error: {}", e),
        })?;

        page.goto(url).await.map_err(|e| ScraperError::Browser {
            url: url.to_string(),
            message: format!("goto error: {}", e),
        })?;

        // Убираем признак автоматизации
        page.evaluate(r#"Object.defineProperty(navigator,'webdriver',{get:()=>undefined})"#).await.ok();

        tokio::time::sleep(tokio::time::Duration::from_millis(self.wait_ms)).await;

        // Кликаем все кнопки "показать ещё" / "load more" и скроллим вниз
        // чтобы подгрузить lazy-loaded контент
        page.evaluate(r#"
            (async function() {
                // Скроллим вниз несколько раз
                for (var i = 0; i < 5; i++) {
                    window.scrollTo(0, document.body.scrollHeight);
                    await new Promise(r => setTimeout(r, 500));
                }
                // Кликаем кнопки "показать ещё" / "load more" до 5 раз
                for (var attempt = 0; attempt < 5; attempt++) {
                    var clicked = false;
                    var btns = document.querySelectorAll('button, a, [role="button"]');
                    btns.forEach(function(btn) {
                        var t = btn.textContent.trim().toLowerCase();
                        if (t.includes('показать ещё') || t.includes('загрузить ещё') ||
                            t.includes('load more') || t.includes('show more') ||
                            t.includes('ещё вакансии') || t.includes('все вакансии') ||
                            t === 'ещё' || t === 'далее') {
                            btn.click();
                            clicked = true;
                        }
                    });
                    if (!clicked) break;
                    await new Promise(r => setTimeout(r, 2000));
                    window.scrollTo(0, document.body.scrollHeight);
                    await new Promise(r => setTimeout(r, 500));
                }
            })()
        "#).await.ok();

        let kw_json = self.inner.keywords_json();
        let js = extraction_js(&kw_json);
        let js_result = page.evaluate(js).await;

        // Дебаг до закрытия браузера
        let json_str = match js_result {
            Ok(val) => val.clone().into_value::<String>()
                .or_else(|_| val.into_value::<serde_json::Value>().map(|v| v.to_string()))
                .unwrap_or_default(),
            Err(e) => {
                eprintln!("[browser debug] JS error: {}", e);
                String::new()
            }
        };

        eprintln!("[browser debug] JS result ({} chars): {}", json_str.len(), &json_str[..json_str.len().min(500)]);

        // Если ничего не нашли — запускаем диагностику
        if json_str.is_empty() || json_str == "[]" || json_str == "null" {
            let dbg_js = debug_js(&kw_json);
            if let Ok(dbg) = page.evaluate(dbg_js).await {
                let s = dbg.into_value::<String>()
                    .or_else(|_| Err(()))
                    .unwrap_or_default();
                eprintln!("[browser debug] page info: {}", s);
            }
        }

        browser.close().await.ok();
        handler_task.abort();

        if !json_str.is_empty() && json_str != "[]" && json_str != "null" {
            self.jobs_from_js_results(&json_str, url)
        } else {
            Ok(vec![])
        }
    }
}

/// JS для извлечения вакансий: ищет элементы с ключевыми словами,
/// поднимается вверх по DOM в поисках ближайшей ссылки.
fn extraction_js(kw_json: &str) -> String {
    let mut s = String::new();
    s.push_str("var __kw=");
    s.push_str(kw_json);
    // Используем raw string — никаких проблем с экранированием Rust
    s.push_str(r#";
var __res=[];
var __seen={};
function __m(t){
    return __kw.some(function(k){
        try{var r=new RegExp('\\b'+k+'\\b','i');return r.test(t);}catch(e){return t.toLowerCase().indexOf(k.toLowerCase())>=0;}
    });
}
function __link(el){
    var c=el;
    for(var i=0;i<15;i++){
        if(!c)break;
        if(c.tagName==='A'&&c.href&&c.href.indexOf('javascript')<0)return c.href;
        if(c.dataset&&c.dataset.href)return c.dataset.href;
        c=c.parentElement;
    }
    return null;
}
document.querySelectorAll('h1,h2,h3,h4,h5,h6,span,p,nobr,b,strong,div').forEach(function(el){
    var t='';
    el.childNodes.forEach(function(n){if(n.nodeType===3)t+=n.textContent;});
    t=t.trim();
    if(t.length<3||t.length>150)return;
    if(!__m(t))return;
    var h=__link(el);
    if(!h)return;
    if(__seen[h])return;
    __seen[h]=true;
    __res.push({title:t,url:h});
});
JSON.stringify(__res);"#);
    s
}

/// Диагностический JS
fn debug_js(kw_json: &str) -> String {
    let mut s = String::new();
    s.push_str("var __kw=");
    s.push_str(kw_json);
    s.push_str(r#";
var info={url:window.location.href,title:document.title,bodyLen:document.body?document.body.innerText.length:0,h3:document.querySelectorAll('h3').length,a:document.querySelectorAll('a[href]').length,matches:[]};
document.querySelectorAll('h1,h2,h3,h4,h5,h6,span,p').forEach(function(el){
    var t=el.textContent.trim();
    if(t.length<3||t.length>150)return;
    __kw.forEach(function(k){
        try{var r=new RegExp('\\b'+k+'\\b','i');if(r.test(t)&&info.matches.length<5)info.matches.push({tag:el.tagName,text:t.substring(0,60)});}catch(e){}
    });
});
JSON.stringify(info);"#);
    s
}

#[async_trait]
impl Scraper for BrowserScraper {
    async fn scrape(&self, url: &str) -> Result<Vec<Job>, ScraperError> {
        self.fetch_jobs(url).await
    }

    fn name(&self) -> &str { "browser" }
}
