use serde_json::{Value, json};

use crate::js::{self, re};

const VERDICT: &str = r"^\W*Verdict:\W*(Ready to ship|Fix first|Can['’]t ship until a person acts)";
const FINDING_HEADING: &str = r"^#+\s*\**([A-Z]\d+)\s*·\s*(?:Blocker|Major|Minor|Note)\b";
const HANDED_OVER: &str = r"^Handed to a person:([\s\S]*?)(?:\n\s*\n|\z)";
const ITEM_ID: &str = r"\b([A-Z]\d+)\s*·";

#[derive(Debug)]
pub struct Summary {
    pub ok: bool,
    pub report: String,
    pub turns: Value,
    pub seconds: i64,
    pub cost_usd: Value,
    pub tokens: Value,
    pub error: Value,
}

fn assistant_text(event: &Value) -> String {
    let content = event["message"]["content"].as_array();
    content
        .into_iter()
        .flatten()
        .filter(|item| item["type"] == "text")
        .filter_map(|item| item["text"].as_str())
        .collect()
}

fn number_or_zero(value: &Value) -> Value {
    match value {
        Value::Null => Value::from(0),
        other => js::number(other),
    }
}

fn tokens_of(event: &Value) -> Value {
    let usage = &event["usage"];
    let writes = &usage["cache_creation"];
    let models = event["modelUsage"].as_object().into_iter().flatten();
    let thinking: f64 = models
        .filter_map(|(_, model)| model["thinkingTokens"].as_f64())
        .sum();
    json!({
        "input": number_or_zero(&usage["input_tokens"]),
        "output": number_or_zero(&usage["output_tokens"]),
        "thinking": thinking as i64,
        "cacheRead": number_or_zero(&usage["cache_read_input_tokens"]),
        "cacheWrite5m": number_or_zero(&writes["ephemeral_5m_input_tokens"]),
        "cacheWrite1h": number_or_zero(&writes["ephemeral_1h_input_tokens"]),
    })
}

pub fn summarise(lines: &[String]) -> Summary {
    let mut out = Summary {
        ok: false,
        report: String::new(),
        turns: Value::from(0),
        seconds: 0,
        cost_usd: Value::from(0),
        tokens: Value::Null,
        error: Value::Null,
    };
    let mut last_text = String::new();
    for line in lines {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if event["type"] == "assistant" {
            let text = assistant_text(&event);
            if !js::trim(&text).is_empty() {
                last_text = text;
            }
        }
        if event["type"] == "result" {
            out.ok = event["subtype"] == "success" && !js::truthy(&event["is_error"]);
            out.report = match event["result"].as_str() {
                Some(result) if !js::trim(result).is_empty() => result.to_string(),
                _ => last_text.clone(),
            };
            out.turns = number_or_zero(&event["num_turns"]);
            let milliseconds = event["duration_ms"].as_f64().unwrap_or(0.0);
            out.seconds = js::round(milliseconds / 1000.0) as i64;
            out.cost_usd = number_or_zero(&event["total_cost_usd"]);
            out.tokens = tokens_of(&event);
            if !out.ok {
                out.error = match &event["subtype"] {
                    Value::Null => Value::from("error"),
                    subtype => subtype.clone(),
                };
            }
        }
    }
    if out.report.is_empty() {
        out.report = last_text;
    }
    out
}

pub fn handed_over(brief: &str) -> Vec<String> {
    let handed = re(HANDED_OVER, "m");
    let section = handed
        .captures(brief)
        .and_then(|caps| caps.get(1))
        .map_or("", |found| found.as_str());
    re(ITEM_ID, "")
        .captures_iter(section)
        .map(|caps| caps[1].to_string())
        .collect()
}

#[derive(Debug)]
pub struct ReportRead {
    pub verdict: Option<String>,
    pub repeated: Vec<String>,
}

pub fn read_report(report: &str, brief: &str) -> ReportRead {
    let handed = handed_over(brief);
    let mut repeated = Vec::new();
    for caps in re(FINDING_HEADING, "m").captures_iter(report) {
        let id = caps[1].to_string();
        if handed.contains(&id) && !repeated.contains(&id) {
            repeated.push(id);
        }
    }
    let verdict = re(VERDICT, "m")
        .captures(report)
        .map(|caps| caps[1].replacen('’', "'", 1));
    ReportRead { verdict, repeated }
}
