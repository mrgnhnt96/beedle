use serde_json::Value;

use crate::js::{self, re};

pub fn filled(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
}

pub fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pattern = format!(r#"\b{name}\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#);
    let caps = re(&pattern, "i").captures(tag)?;
    (1..=3)
        .find_map(|group| caps.get(group))
        .map(|found| found.as_str())
}

pub fn tags<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    re(&format!(r"<{name}\b[^>]*>"), "i")
        .find_iter(html)
        .map(|found| found.as_str())
        .collect()
}

pub fn meta<'a>(html: &'a str, key: &str) -> Option<&'a str> {
    let named = |tag: &&str, name: &str| attr(tag, name).is_some_and(|v| v.to_lowercase() == key);
    let tag = tags(html, "meta")
        .into_iter()
        .find(|tag| named(tag, "name") || named(tag, "property"))?;
    attr(tag, "content")
}

pub fn link<'a>(html: &'a str, rel: &str) -> Option<&'a str> {
    let has_rel = |tag: &&str| {
        attr(tag, "rel").is_some_and(|value| {
            value
                .to_lowercase()
                .split(js::is_space)
                .any(|name| name == rel)
        })
    };
    let tag = tags(html, "link").into_iter().find(has_rel)?;
    attr(tag, "href")
}

pub fn head(html: &str) -> &str {
    re(r"<head\b[\s\S]*?</head>", "i")
        .find(html)
        .map_or("", |found| found.as_str())
}

pub fn visible_text(html: &str) -> String {
    let body = re(r"<body\b[\s\S]*$", "i")
        .find(html)
        .map_or(html, |found| found.as_str());
    let hidden = concat!(
        r"<(?:script\b[\s\S]*?</script>|style\b[\s\S]*?</style>|noscript\b[\s\S]*?</noscript>",
        r"|svg\b[\s\S]*?</svg>|template\b[\s\S]*?</template>)"
    );
    let text = re(hidden, "i").replace_all(body, " ");
    let text = re(r"<!--[\s\S]*?-->", "").replace_all(&text, " ");
    let text = re(r"<[^>]+>", "").replace_all(&text, " ");
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&quot;", "\"");
    js::trim(&re(r"\s+", "").replace_all(&text, " ")).to_string()
}

#[derive(Default)]
pub struct JsonLd {
    pub items: Vec<Value>,
    pub invalid: usize,
}

impl JsonLd {
    fn take(&mut self, data: Value) {
        match data {
            Value::Array(items) => self.items.extend(items),
            Value::Null => self.invalid += 1,
            other => self.take_graph(other),
        }
    }

    fn take_graph(&mut self, data: Value) {
        match data.get("@graph") {
            None | Some(Value::Null) => self.items.push(data),
            Some(Value::Array(items)) => self.items.extend(items.iter().cloned()),
            Some(Value::String(_)) => {}
            Some(_) => self.invalid += 1,
        }
    }
}

pub fn json_ld(html: &str) -> JsonLd {
    let mut found = JsonLd::default();
    let blocks = re(
        r"<script\b[^>]*application/ld\+json[^>]*>[\s\S]*?</script>",
        "i",
    );
    for block in blocks.find_iter(html) {
        let opened = re(r"^<script\b[^>]*>", "i").replace(block.as_str(), "");
        let body = re(r"</script>$", "i").replace(&opened, "");
        match serde_json::from_str(&body) {
            Ok(data) => found.take(data),
            Err(_) => found.invalid += 1,
        }
    }
    found
}

fn type_name(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(_) => "[object Object]".to_string(),
        other => other.to_string(),
    }
}

pub fn types_of(item: &Value) -> Vec<String> {
    let names = match item.get("@type") {
        Some(Value::Array(names)) => names.iter().collect(),
        Some(name) => vec![name],
        None => Vec::new(),
    };
    names
        .into_iter()
        .filter(|name| js::truthy(name))
        .map(type_name)
        .collect()
}
