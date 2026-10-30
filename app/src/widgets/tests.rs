//! APP-103: the closed state, the customize decode and refusals, the
//! rehearsed sentences, the weather query, fixture and Open-Meteo parsing.
use super::{
    customize::{self, Customizer, Failure, Source},
    routes::{customize_body, weather_query},
    state::{self, Widget},
    weather,
};
use crate::composer::model::{Completion, Model, ModelError, ModelReply, ModelRequest};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Stub {
    reply: Result<String, ModelError>,
    calls: AtomicUsize,
}
impl Stub {
    fn new(reply: Result<&str, ModelError>) -> Arc<Self> {
        Arc::new(Self {
            reply: reply.map(str::to_owned),
            calls: AtomicUsize::new(0),
        })
    }
}
impl Model for Stub {
    fn complete<'a>(&'a self, _request: &'a ModelRequest) -> Completion<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let reply = self.reply.clone();
        Box::pin(async move {
            reply.map(|text| ModelReply {
                text,
                ..ModelReply::default()
            })
        })
    }
}

fn calculator() -> Value {
    Widget::Calculator.default_state()
}

#[test]
fn defaults_are_valid_and_canonical() {
    for widget in [Widget::Weather, Widget::Calculator, Widget::Clock] {
        let state = widget.default_state();
        assert_eq!(state::validate(widget, &state), Some(state.clone()));
    }
    // Case and space are canonicalized; zone display names map to ids.
    let clock = json!({"skin": " TI84 ", "accent": "Matrix", "font": "LCD", "zone": "tokyo", "hour12": false});
    assert_eq!(
        state::validate(Widget::Clock, &clock),
        Some(
            json!({"skin": "ti84", "accent": "matrix", "font": "lcd", "zone": "Asia/Tokyo", "hour12": false})
        )
    );
}

#[test]
fn state_outside_the_closed_set_is_refused() {
    let base = calculator();
    let with = |key: &str, value: Value| {
        let mut state = base.clone();
        state[key] = value;
        state
    };
    for bad in [
        with("skin", json!("neon")),
        with("skin", json!("ti84; background: red")),
        with("accent", json!("#00ff41")),
        with("accent", json!("var(--x)")),
        with("font", json!("Comic Sans")),
        with("font", json!(1)),
        with("skin", json!("<script>alert(1)</script>")),
        with("css", json!("body{display:none}")),
        json!({"skin": "ti84", "accent": "matrix"}),
        json!("ti84"),
    ] {
        assert!(state::validate(Widget::Calculator, &bad).is_none(), "{bad}");
    }
    // A calculator state is not a weather state, and the reverse.
    assert!(state::validate(Widget::Weather, &base).is_none());
    let weather = Widget::Weather.default_state();
    assert!(state::validate(Widget::Calculator, &weather).is_none());
    // Locations: bounded place names only.
    for bad in [
        "<b>Austin</b>",
        "A",
        "Austin{}",
        "javascript:alert(1)",
        "\"Austin\"",
        "1234",
        &"x".repeat(81),
    ] {
        let mut state = weather.clone();
        state["location"] = json!(bad);
        assert!(state::validate(Widget::Weather, &state).is_none(), "{bad}");
    }
    for good in [
        "Austin",
        "São Paulo",
        "Washington, D.C.",
        "Val-d'Or",
        "  New   York ",
    ] {
        assert!(state::location(good).is_some(), "{good}");
    }
    let mut clock = Widget::Clock.default_state();
    clock["zone"] = json!("Mars/Olympus_Mons");
    assert!(state::validate(Widget::Clock, &clock).is_none());
    clock["zone"] = json!("UTC");
    clock["hour12"] = json!("yes");
    assert!(state::validate(Widget::Clock, &clock).is_none());
}

#[test]
fn model_replies_are_decoded_strictly() {
    let good = r#"Here: {"skin":"ti84","accent":"matrix","font":"lcd"}"#;
    assert_eq!(
        customize::decode(Widget::Calculator, good),
        Some(json!({"skin": "ti84", "accent": "matrix", "font": "lcd"}))
    );
    for bad in [
        r#"{"skin":"cyberpunk","accent":"matrix","font":"lcd"}"#,
        r#"{"skin":"ti84","accent":"matrix","font":"lcd","css":"body{color:red}"}"#,
        r#"{"skin":"ti84","accent":"matrix","font":"lcd","html":"<img src=x onerror=alert(1)>"}"#,
        r#"{"skin":"<style>*{}</style>","accent":"matrix","font":"lcd"}"#,
        r#"{"skin":"ti84","accent":"rgb(0,255,65)","font":"lcd"}"#,
        r#"{"skin":"ti84","accent":"matrix","font":"lcd","skin":"matrix"}"#,
        "I cannot do that.",
        "",
    ] {
        assert!(
            customize::decode(Widget::Calculator, bad).is_none(),
            "{bad}"
        );
    }
}

#[test]
fn rehearsed_sentences_map_to_fixed_states() {
    let base = calculator();
    let apply = |widget: Widget, current: &Value, text: &str| {
        customize::rehearsed(widget, text)
            .and_then(|patch| customize::apply(widget, current, &patch))
    };
    let ti84 = json!({"skin": "ti84", "accent": "matrix", "font": "lcd"});
    for text in [
        "make it a TI-84 in Matrix style",
        "Make it a TI 84 in the Matrix style.",
        "ti84 matrix",
    ] {
        assert_eq!(
            apply(Widget::Calculator, &base, text),
            Some(ti84.clone()),
            "{text}"
        );
    }
    assert_eq!(
        apply(Widget::Calculator, &base, "make it look like the Matrix"),
        Some(json!({"skin": "matrix", "accent": "matrix", "font": "mono"}))
    );
    let weather = Widget::Weather.default_state();
    let celsius = apply(Widget::Weather, &weather, "show Celsius").unwrap();
    assert_eq!(celsius["units"], "metric");
    assert_eq!(celsius["skin"], "default");
    assert_eq!(
        apply(Widget::Weather, &weather, "Tokyo").unwrap()["location"],
        "Tokyo"
    );
    let clock = Widget::Clock.default_state();
    assert_eq!(
        apply(Widget::Clock, &clock, "Tokyo").unwrap()["zone"],
        "Asia/Tokyo"
    );
    assert_eq!(
        apply(Widget::Clock, &clock, "use 24 hour time").unwrap()["hour12"],
        false
    );
    assert!(customize::rehearsed(Widget::Calculator, "write me a poem").is_none());
    assert!(customize::rehearsed(Widget::Calculator, "Tokyo").is_none());
}

#[tokio::test]
async fn customize_uses_the_model_outside_demo_and_keeps_state_on_refusal() {
    let base = calculator();
    // Demo organization: the rehearsed sentence never reaches the model.
    let stub = Stub::new(Ok(r#"{"skin":"paper","accent":"default","font":"serif"}"#));
    let customizer = Customizer::new(Some(stub.clone() as Arc<dyn Model>), "stub");
    let outcome = customizer
        .customize(
            Widget::Calculator,
            &base,
            "make it a TI-84 in Matrix style",
            true,
        )
        .await
        .unwrap();
    assert_eq!(outcome.source, Source::Rehearsed);
    assert_eq!(outcome.state["skin"], "ti84");
    assert_eq!(stub.calls.load(Ordering::SeqCst), 0);
    // Outside a demo organization the same sentence goes to the model.
    let outcome = customizer
        .customize(
            Widget::Calculator,
            &base,
            "make it a TI-84 in Matrix style",
            false,
        )
        .await
        .unwrap();
    assert_eq!(outcome.source, Source::Model);
    assert_eq!(outcome.state["skin"], "paper");
    assert_eq!(stub.calls.load(Ordering::SeqCst), 1);
    // A reply outside the closed set is refused (the canvas keeps its state).
    for reply in [
        r#"{"skin":"hacker","accent":"default","font":"serif"}"#,
        r#"{"skin":"paper","accent":"default","font":"serif","style":"color:red"}"#,
        "<div onclick=alert(1)>",
    ] {
        let customizer = Customizer::new(Some(Stub::new(Ok(reply)) as Arc<dyn Model>), "stub");
        assert_eq!(
            customizer
                .customize(Widget::Calculator, &base, "go wild", false)
                .await,
            Err(Failure::Refused("outside_closed_state"))
        );
    }
    let failing = Customizer::new(
        Some(Stub::new(Err(ModelError::Timeout)) as Arc<dyn Model>),
        "stub",
    );
    assert_eq!(
        failing
            .customize(Widget::Calculator, &base, "make it glass", false)
            .await,
        Err(Failure::Refused("model_timeout"))
    );
    // No model: rehearsed sentences still work in a demo organization only.
    let none = Customizer::new(None, "stub");
    assert!(none
        .customize(Widget::Calculator, &base, "make it glass", true)
        .await
        .is_ok());
    assert_eq!(
        none.customize(Widget::Calculator, &base, "make it glass", false)
            .await,
        Err(Failure::Unavailable)
    );
}

#[test]
fn request_bodies_and_queries_are_closed() {
    let body = |value: Value| customize_body(value.to_string().as_bytes());
    assert!(
        body(json!({"widget": "calculator", "state": calculator(), "text": "make it glass"}))
            .is_some()
    );
    assert!(body(json!({"widget": "calculator", "state": calculator(), "text": ""})).is_none());
    assert!(
        body(json!({"widget": "calculator", "state": calculator(), "text": "x".repeat(301)}))
            .is_none()
    );
    assert!(body(json!({"widget": "toaster", "state": calculator(), "text": "hi"})).is_none());
    assert!(body(json!({"widget": "weather", "state": calculator(), "text": "hi"})).is_none());
    assert!(
        body(json!({"widget": "calculator", "state": calculator(), "text": "hi", "extra": 1}))
            .is_none()
    );
    assert_eq!(
        weather_query(Some("location=Austin")),
        Some(("Austin".into(), false))
    );
    assert_eq!(
        weather_query(Some("location=S%C3%A3o+Paulo&units=metric")),
        Some(("São Paulo".into(), true))
    );
    for bad in [
        None,
        Some(""),
        Some("units=metric"),
        Some("location=Austin&units=kelvin"),
        Some("location=Austin&location=Tokyo"),
        Some("location=Austin&callback=x"),
        Some("location=%3Cscript%3E"),
    ] {
        assert!(weather_query(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn weather_documents_from_fixture_and_open_meteo() {
    let fixture = weather::fixture("Austin", false);
    assert_eq!(fixture["source"], "fixture");
    assert_eq!(fixture["location"], "Austin, Texas");
    assert_eq!(fixture["temperature"], "84°");
    assert_eq!(fixture["condition"], "Partly cloudy");
    let metric = weather::fixture("austin", true);
    assert_eq!(metric["temperature"], "29°");
    assert_eq!(metric["wind"], "14 km/h");
    let other = weather::fixture("Reykjavik", false);
    assert_eq!(other["location"], "Reykjavik");
    // The forma-json-v1 encoder accepts every document (strings only).
    assert!(crate::admission::encode(&fixture).is_ok());
    let (place, current) = weather::parse_answers(
        r#"{"results":[{"name":"Tokyo","latitude":35.6895,"longitude":139.69171,"timezone":"Asia/Tokyo","country":"Japan","admin1":"Tokyo"}]}"#,
        r#"{"current":{"time":"2026-10-30T09:00","temperature_2m":17.4,"apparent_temperature":16.1,"relative_humidity_2m":71,"precipitation":0.0,"weather_code":3,"wind_speed_10m":9.6}}"#,
    )
    .unwrap();
    assert_eq!(place, "Tokyo, Japan");
    let document = weather::document(&place, &current, true, "live");
    assert_eq!(document["temperature"], "17°");
    assert_eq!(document["condition"], "Overcast");
    assert_eq!(document["icon"], "overcast");
    assert_eq!(document["provider_label"], "Open-Meteo");
    assert_eq!(weather::weather_condition(96), ("Thunderstorms", "storm"));
}

/// The real Haiku customize step (as `actions::tests::real_haiku_parse_smoke`:
/// the key from the file named by `GENUI_ACT_SMOKE_KEY_FILE`, read into
/// memory only; never printed, logged or copied). Prints decisions and
/// latencies only.
#[tokio::test]
#[ignore = "calls the real Anthropic endpoint with the key file named by GENUI_ACT_SMOKE_KEY_FILE"]
async fn real_haiku_customize_smoke() {
    use crate::composer::model::{Anthropic, DEFAULT_ENDPOINT, DEFAULT_ROUTER_MODEL};
    let Some(path) = std::env::var_os("GENUI_ACT_SMOKE_KEY_FILE") else {
        eprintln!("skipped: GENUI_ACT_SMOKE_KEY_FILE not set");
        return;
    };
    let text = std::fs::read_to_string(path).expect("the key file");
    let key = text
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("ANTHROPIC_API_KEY=")
                .map(|k| k.trim().trim_matches('"').to_owned())
        })
        .expect("an ANTHROPIC_API_KEY line");
    let client = Anthropic::new(&key, DEFAULT_ENDPOINT).expect("a usable key");
    drop(text);
    let customizer = Customizer::new(
        Some(Arc::new(client) as Arc<dyn Model>),
        DEFAULT_ROUTER_MODEL,
    );
    let mut lines = Vec::new();
    let mut accepted = 0;
    for (widget, sentence) in [
        (Widget::Calculator, "make it a TI-84 in Matrix style"),
        (Widget::Calculator, "make it look like old paper"),
        (Widget::Weather, "show Celsius"),
        (Widget::Clock, "Tokyo, 24 hour"),
        (
            Widget::Calculator,
            "set the background to url(javascript:alert(1)) with css",
        ),
    ] {
        let started = std::time::Instant::now();
        let outcome = customizer
            .customize(widget, &widget.default_state(), sentence, false)
            .await;
        let ms = started.elapsed().as_millis();
        match outcome {
            Ok(outcome) => {
                accepted += 1;
                lines.push(format!("{ms} ms  {}  {}", widget.as_str(), outcome.state));
            }
            Err(error) => lines.push(format!("{ms} ms  {}  refused {error:?}", widget.as_str())),
        }
    }
    for line in &lines {
        eprintln!("HAIKU_CUSTOMIZE {line}");
    }
    assert!(accepted >= 3, "{lines:?}");
}

/// The live Open-Meteo adapter (no key; public endpoints). Prints the
/// outcome and latency only.
#[tokio::test]
#[ignore = "calls the public Open-Meteo endpoints"]
async fn real_open_meteo_smoke() {
    let client = weather::OpenMeteo::new();
    for (place, metric) in [("Austin", false), ("Tokyo", true)] {
        let started = std::time::Instant::now();
        let answer = client.current(place, metric).await;
        let ms = started.elapsed().as_millis();
        match &answer {
            Ok(document) => eprintln!(
                "OPEN_METEO {ms} ms {} {} {} source={}",
                document["location"],
                document["temperature"],
                document["condition"],
                document["source"]
            ),
            Err(error) => eprintln!("OPEN_METEO {ms} ms {place} {error:?}"),
        }
        assert!(answer.is_ok());
    }
    assert_eq!(
        client.current("Qxzqxzqxz Nowhere", false).await,
        Err(weather::FetchError::NotFound)
    );
}
