use std::collections::{BTreeMap, HashSet};
use std::hint::black_box;
use std::time::Instant;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::cleanup::assess_cleanup;
use crate::settings::Style;

const PANEL_VERSION: &str = "cleanup-fidelity-v1";
const LANGUAGES: [&str; 3] = ["en", "nl", "mixed"];
const HARMFUL: [&str; 8] = [
    "amount",
    "date",
    "recipient",
    "negation",
    "question",
    "addition",
    "translation",
    "roles",
];
const BENIGN: [&str; 6] = [
    "punctuation",
    "formatting",
    "dictionary",
    "number",
    "correction",
    "question",
];
const REPETITIONS: usize = 30;

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    split: &'static str,
    language: &'static str,
    category: &'static str,
    harmful: bool,
    raw: String,
    candidate: String,
    style: Style,
    dictionary: Vec<String>,
}

struct Family {
    id: &'static str,
    item_en: &'static str,
    item_nl: &'static str,
    name: &'static str,
    other: &'static str,
    number: u16,
    spoken_en: &'static str,
    spoken_nl: &'static str,
    day_en: &'static str,
    day_nl: &'static str,
    other_day_en: &'static str,
    other_day_nl: &'static str,
    dictionary_raw: &'static str,
    dictionary_saved: &'static str,
}

const FAMILIES: [Family; 8] = [
    Family {
        id: "harbor",
        item_en: "harbor inventory",
        item_nl: "haveninventaris",
        name: "Amina",
        other: "Bram",
        number: 27,
        spoken_en: "twenty seven",
        spoken_nl: "zevenentwintig",
        day_en: "Monday",
        day_nl: "maandag",
        other_day_en: "Wednesday",
        other_day_nl: "woensdag",
        dictionary_raw: "maple board",
        dictionary_saved: "MapleBoard",
    },
    Family {
        id: "orchard",
        item_en: "orchard schedule",
        item_nl: "boomgaardschema",
        name: "Celia",
        other: "Daan",
        number: 83,
        spoken_en: "eighty three",
        spoken_nl: "drieëntachtig",
        day_en: "Friday",
        day_nl: "vrijdag",
        other_day_en: "Sunday",
        other_day_nl: "zondag",
        dictionary_raw: "birch cafe",
        dictionary_saved: "BirchCafé",
    },
    Family {
        id: "observatory",
        item_en: "telescope invoice",
        item_nl: "telescoopfactuur",
        name: "Elias",
        other: "Farah",
        number: 145,
        spoken_en: "one hundred forty five",
        spoken_nl: "honderdvijfenveertig",
        day_en: "Tuesday",
        day_nl: "dinsdag",
        other_day_en: "Thursday",
        other_day_nl: "donderdag",
        dictionary_raw: "cedar desk",
        dictionary_saved: "CedarDesk",
    },
    Family {
        id: "theater",
        item_en: "stage booking",
        item_nl: "podiumboeking",
        name: "Greta",
        other: "Hugo",
        number: 226,
        spoken_en: "two hundred twenty six",
        spoken_nl: "tweehonderdzesentwintig",
        day_en: "Wednesday",
        day_nl: "woensdag",
        other_day_en: "Saturday",
        other_day_nl: "zaterdag",
        dictionary_raw: "willow page",
        dictionary_saved: "WillowPage",
    },
    Family {
        id: "clinic",
        item_en: "clinic brochure",
        item_nl: "kliniekbrochure",
        name: "Idris",
        other: "Juna",
        number: 319,
        spoken_en: "three hundred nineteen",
        spoken_nl: "driehonderdnegentien",
        day_en: "Thursday",
        day_nl: "donderdag",
        other_day_en: "Monday",
        other_day_nl: "maandag",
        dictionary_raw: "aspen plan",
        dictionary_saved: "AspenPlan",
    },
    Family {
        id: "museum",
        item_en: "exhibit catalog",
        item_nl: "tentoonstellingscatalogus",
        name: "Kiran",
        other: "Lotte",
        number: 472,
        spoken_en: "four hundred seventy two",
        spoken_nl: "vierhonderdtweeënzeventig",
        day_en: "Saturday",
        day_nl: "zaterdag",
        other_day_en: "Tuesday",
        other_day_nl: "dinsdag",
        dictionary_raw: "hazel cafe",
        dictionary_saved: "HazelCafé",
    },
    Family {
        id: "workshop",
        item_en: "workshop shipment",
        item_nl: "werkplaatszending",
        name: "Mira",
        other: "Nolan",
        number: 583,
        spoken_en: "five hundred eighty three",
        spoken_nl: "vijfhonderddrieëntachtig",
        day_en: "Sunday",
        day_nl: "zondag",
        other_day_en: "Friday",
        other_day_nl: "vrijdag",
        dictionary_raw: "rowan sheet",
        dictionary_saved: "RowanSheet",
    },
    Family {
        id: "greenhouse",
        item_en: "greenhouse checklist",
        item_nl: "kascontrolelijst",
        name: "Orla",
        other: "Pieter",
        number: 698,
        spoken_en: "six hundred ninety eight",
        spoken_nl: "zeshonderdachtennegentig",
        day_en: "Monday",
        day_nl: "maandag",
        other_day_en: "Thursday",
        other_day_nl: "donderdag",
        dictionary_raw: "elm ledger",
        dictionary_saved: "ElmLedger",
    },
];

fn panel() -> Vec<Case> {
    let mut cases = Vec::new();
    for (family_index, family) in FAMILIES.iter().enumerate() {
        for language in LANGUAGES {
            let dutch = language == "nl";
            let mixed = language == "mixed";
            let item = if dutch || mixed {
                family.item_nl
            } else {
                family.item_en
            };
            let day = if dutch { family.day_nl } else { family.day_en };
            let other_day = if dutch {
                family.other_day_nl
            } else {
                family.other_day_en
            };
            let send = if dutch || mixed { "stuur" } else { "send" };
            let to = if dutch || mixed { "naar" } else { "to" };
            let on = if dutch { "op" } else { "on" };
            let amount_raw = format!("{send} {} euro {to} {}", family.number, family.name);
            let date_raw = format!("{send} {item} {on} {day}");
            let recipient_raw = format!("{send} {item} {to} {}", family.name);
            let negative_raw = if dutch {
                format!("publiceer {item} niet voor {}", family.name)
            } else {
                format!("do not publish {item} for {}", family.name)
            };
            let question_raw = if dutch {
                format!("kun je {item} {to} {} sturen", family.name)
            } else {
                format!("can you send {item} {to} {}", family.name)
            };
            let role_raw = if dutch {
                format!("{} betaalt {} voor {item}", family.name, family.other)
            } else {
                format!("{} pays {} for {item}", family.name, family.other)
            };
            let harmful = [
                (
                    amount_raw.clone(),
                    format!("{send} {} euro {to} {}.", family.number + 1, family.name),
                ),
                (date_raw.clone(), format!("{send} {item} {on} {other_day}.")),
                (
                    recipient_raw.clone(),
                    format!("{send} {item} {to} {}.", family.other),
                ),
                (
                    negative_raw,
                    if dutch {
                        format!("publiceer {item} voor {}.", family.name)
                    } else {
                        format!("do publish {item} for {}.", family.name)
                    },
                ),
                (
                    question_raw.clone(),
                    if dutch {
                        format!("je kunt {item} {to} {} sturen.", family.name)
                    } else {
                        format!("you can send {item} {to} {}.", family.name)
                    },
                ),
                (
                    recipient_raw.clone(),
                    format!(
                        "{recipient_raw} {}.",
                        if dutch { "morgen" } else { "tomorrow" }
                    ),
                ),
                (
                    date_raw.clone(),
                    if dutch {
                        format!("{send} {item} on {}.", family.day_en)
                    } else {
                        format!("{send} {item} op {}.", family.day_nl)
                    },
                ),
                (
                    role_raw,
                    if dutch {
                        format!("{} betaalt {} voor {item}.", family.other, family.name)
                    } else {
                        format!("{} pays {} for {item}.", family.other, family.name)
                    },
                ),
            ];
            let filler = if dutch || mixed { "euh" } else { "uh" };
            let then = if dutch { "daarna" } else { "then" };
            let archive = if dutch || mixed {
                "archiveer"
            } else {
                "archive"
            };
            let correction = if dutch { "nee wacht" } else { "no wait" };
            let spoken = if dutch || mixed {
                family.spoken_nl
            } else {
                family.spoken_en
            };
            let save = if dutch || mixed { "bewaar" } else { "save" };
            let benign = [
                (
                    format!("{filler} {recipient_raw}"),
                    format!("{recipient_raw}."),
                ),
                (
                    format!("{recipient_raw} {then} {archive} {item}"),
                    format!("{recipient_raw}.\n\n{then} {archive} {item}."),
                ),
                (
                    format!("{save} {item} in {}", family.dictionary_raw),
                    format!("{save} {item} in {}.", family.dictionary_saved),
                ),
                (
                    format!("{send} {spoken} euro {to} {}", family.name),
                    format!("{amount_raw}."),
                ),
                (
                    format!("{date_raw} {correction} {other_day}"),
                    format!("{send} {item} {on} {other_day}."),
                ),
                (question_raw.clone(), format!("{question_raw}?")),
            ];
            for (harmful, categories, proposals) in [
                (true, HARMFUL.as_slice(), harmful.as_slice()),
                (false, BENIGN.as_slice(), benign.as_slice()),
            ] {
                for (category, (raw, candidate)) in categories.iter().zip(proposals) {
                    cases.push(Case {
                        id: format!(
                            "{}-{language}-{}-{category}",
                            family.id,
                            if harmful { "harmful" } else { "benign" }
                        ),
                        family: family.id.into(),
                        split: if family_index < 2 {
                            "development"
                        } else {
                            "held_out"
                        },
                        language,
                        category,
                        harmful,
                        raw: raw.clone(),
                        candidate: candidate.clone(),
                        style: Style::Neutral,
                        dictionary: if *category == "dictionary" {
                            vec![family.dictionary_saved.into()]
                        } else {
                            Vec::new()
                        },
                    });
                }
            }
        }
    }
    cases
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn fixture_hash(cases: &[Case]) -> String {
    digest(&serde_json::to_vec(cases).unwrap())
}

fn historical_accepts(raw: &str, candidate: &str) -> bool {
    fn words(text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_lowercase)
            .collect()
    }
    let candidate = candidate.trim();
    let source: HashSet<_> = words(raw).into_iter().collect();
    let output = words(candidate);
    let share = if output.is_empty() {
        1.0
    } else {
        output.iter().filter(|word| source.contains(*word)).count() as f32 / output.len() as f32
    };
    !candidate.is_empty()
        && candidate.chars().count() <= raw.chars().count() * 2 + 40
        && share >= 0.6
}

fn rows(cases: &[Case], baseline: bool) -> Vec<Value> {
    cases.iter().map(|case| {
        let (accepted, reason_code) = if baseline {
            let accepted = historical_accepts(&case.raw, &case.candidate);
            (accepted, if accepted { "legacy_accepted" } else { "legacy_rejected" })
        } else {
            let result = assess_cleanup(&case.raw, &case.candidate, case.style, &case.dictionary);
            (result.accepted, result.reason_code)
        };
        json!({"id":case.id,"family":case.family,"split":case.split,"language":case.language,"category":case.category,"harmful":case.harmful,"accepted":accepted,"abstained":false,"reason_code":reason_code,"raw_chars":case.raw.chars().count(),"candidate_chars":case.candidate.chars().count(),"raw_sha256":digest(case.raw.as_bytes()),"candidate_sha256":digest(case.candidate.as_bytes())})
    }).collect()
}

fn metric(numerator: usize, denominator: usize) -> Value {
    json!({"numerator":numerator,"denominator":denominator,"rate":if denominator == 0 {None} else {Some(numerator as f64 / denominator as f64)}})
}

fn language_summary(rows: &[Value], language: &str) -> Value {
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| row["language"] == language && row["split"] == "held_out")
        .collect();
    let harmful: Vec<_> = selected
        .iter()
        .filter(|row| row["harmful"] == true)
        .collect();
    let benign: Vec<_> = selected
        .iter()
        .filter(|row| row["harmful"] == false)
        .collect();
    let mut reasons = BTreeMap::new();
    for row in &selected {
        *reasons
            .entry(row["reason_code"].as_str().unwrap())
            .or_insert(0_usize) += 1;
    }
    json!({"harmful_acceptance":metric(harmful.iter().filter(|row| row["accepted"] == true).count(),harmful.len()),"benign_rejection":metric(benign.iter().filter(|row| row["accepted"] == false).count(),benign.len()),"abstention":metric(selected.iter().filter(|row| row["abstained"] == true).count(),selected.len()),"reason_counts":reasons})
}

fn timing_ids(cases: &[Case]) -> Vec<String> {
    cases
        .iter()
        .filter(|case| {
            let Some(index) = FAMILIES.iter().position(|family| family.id == case.family) else {
                return false;
            };
            index >= 2
                && case.category
                    == if case.harmful {
                        HARMFUL[(index - 2) % HARMFUL.len()]
                    } else {
                        BENIGN[(index - 2) % BENIGN.len()]
                    }
        })
        .map(|case| case.id.clone())
        .collect()
}

fn percentile(samples: &[f64], fraction: f64) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = (sorted.len() - 1) as f64 * fraction;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    sorted[lower] + (sorted[upper] - sorted[lower]) * position.fract()
}

fn timing_stats(samples: &[f64]) -> Value {
    json!({"samples":samples.len(),"p50_ms":percentile(samples,0.5),"p95_ms":percentile(samples,0.95)})
}

fn timed(work: impl FnOnce() -> bool) -> f64 {
    let started = Instant::now();
    black_box(work());
    started.elapsed().as_secs_f64() * 1000.0
}

fn timing(cases: &[Case]) -> Value {
    let ids = timing_ids(cases);
    let mut samples = Vec::new();
    for case in cases.iter().filter(|case| ids.contains(&case.id)) {
        black_box(historical_accepts(&case.raw, &case.candidate));
        black_box(assess_cleanup(
            &case.raw,
            &case.candidate,
            case.style,
            &case.dictionary,
        ));
        for repetition in 0..REPETITIONS {
            let baseline = || historical_accepts(black_box(&case.raw), black_box(&case.candidate));
            let candidate = || {
                assess_cleanup(
                    black_box(&case.raw),
                    black_box(&case.candidate),
                    case.style,
                    black_box(&case.dictionary),
                )
                .accepted
            };
            let (baseline_ms, candidate_ms) = if repetition % 2 == 0 {
                (timed(baseline), timed(candidate))
            } else {
                let candidate_ms = timed(candidate);
                (timed(baseline), candidate_ms)
            };
            samples.push(json!({"id":case.id,"language":case.language,"repetition":repetition,"baseline_ms":baseline_ms,"candidate_ms":candidate_ms,"added_ms":candidate_ms-baseline_ms}));
        }
    }
    let languages: BTreeMap<_,_> = LANGUAGES.into_iter().map(|language| {
        let selected:Vec<_>=samples.iter().filter(|row|row["language"]==language).collect();
        let values=|key:&str|selected.iter().map(|row|row[key].as_f64().unwrap()).collect::<Vec<_>>();
        (language,json!({"baseline":timing_stats(&values("baseline_ms")),"candidate":timing_stats(&values("candidate_ms")),"added":timing_stats(&values("added_ms"))}))
    }).collect();
    json!({"kind":"deterministic_cpu_overhead","profile":"cargo test debug, unoptimized","concurrent_load":"uncontrolled","unit":"milliseconds","percentile":"linear_type_7","warmups_per_case":1,"repetitions_per_case":REPETITIONS,"paired_order":"alternating","case_ids":ids,"languages":languages,"samples":samples,"live_semantic_latency":"unmeasured"})
}

fn mock_verification_timing() -> Value {
    let mut samples = Vec::new();
    let mut warmups = Vec::new();
    for language in LANGUAGES {
        warmups.push(json!({"language":language,"baseline":crate::cleanup_verification::cleanup_pipeline_sample("disabled",language),"candidate":crate::cleanup_verification::cleanup_pipeline_sample("accepted",language)}));
        for repetition in 0..REPETITIONS {
            let baseline =
                || crate::cleanup_verification::cleanup_pipeline_sample("disabled", language);
            let candidate =
                || crate::cleanup_verification::cleanup_pipeline_sample("accepted", language);
            let (baseline, candidate) = if repetition % 2 == 0 {
                (baseline(), candidate())
            } else {
                let candidate = candidate();
                (baseline(), candidate)
            };
            let baseline_ms = baseline["elapsed_ms"].as_f64().unwrap();
            let candidate_ms = candidate["elapsed_ms"].as_f64().unwrap();
            samples.push(json!({"language":language,"repetition":repetition,"baseline_ms":baseline_ms,"candidate_ms":candidate_ms,"added_ms":candidate_ms-baseline_ms,"baseline_generation_calls":baseline["generation_calls"],"baseline_verification_calls":baseline["verification_calls"],"candidate_generation_calls":candidate["generation_calls"],"candidate_verification_calls":candidate["verification_calls"]}));
        }
    }
    let languages: BTreeMap<_, _> = LANGUAGES.into_iter().map(|language| {
        let selected:Vec<_>=samples.iter().filter(|row|row["language"]==language).collect();
        let values=|key:&str|selected.iter().map(|row|row[key].as_f64().unwrap()).collect::<Vec<_>>();
        let calls=|key:&str|selected.iter().map(|row|row[key].as_u64().unwrap()).sum::<u64>();
        (language,json!({"baseline":timing_stats(&values("baseline_ms")),"candidate":timing_stats(&values("candidate_ms")),"added":timing_stats(&values("added_ms")),"measured_calls":{"baseline_generation":calls("baseline_generation_calls"),"baseline_verification":calls("baseline_verification_calls"),"candidate_generation":calls("candidate_generation_calls"),"candidate_verification":calls("candidate_verification_calls")}}))
    }).collect();
    json!({"kind":"mock_verifier_cleanup_overhead","evidence":"actual loopback mock requests; programmed answers establish no model qualification","unit":"milliseconds","percentile":"linear_type_7","warmups_per_language":1,"repetitions_per_language":REPETITIONS,"paired_order":"alternating","languages":languages,"samples":samples,"excluded_warmup_records":warmups,"live_stop_to_result_latency":"unmeasured"})
}

fn mock_scenarios() -> Vec<Value> {
    let mut scenarios = crate::cleanup_verification::cleanup_pipeline_report()
        .as_array()
        .unwrap()
        .clone();
    for row in &mut scenarios {
        row["language"] = json!("en");
    }
    let ids = scenarios
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    for language in ["nl", "mixed"] {
        for id in &ids {
            let mut row = crate::cleanup_verification::cleanup_pipeline_sample(id, language);
            row["language"] = json!(language);
            scenarios.push(row);
        }
    }
    scenarios
}

fn manifest(cases: &[Case]) -> Value {
    json!({"panel_version":PANEL_VERSION,"fixture_sha256":fixture_hash(cases),"total_cases":cases.len(),"development_families":FAMILIES[..2].iter().map(|family|family.id).collect::<Vec<_>>(),"held_out_families":FAMILIES[2..].iter().map(|family|family.id).collect::<Vec<_>>(),"languages":LANGUAGES,"held_out_per_language":{"harmful":48,"benign":36},"timing_case_ids":timing_ids(cases),"criteria":{"mandatory_regressions":"pass","held_out_harmful_acceptance":0,"benign_rejection":"report every rejection without fitting a cutoff","production_model_qualification":"unavailable"}})
}

#[test]
fn fixture_groups_and_denominators_are_frozen() {
    let cases = panel();
    assert_eq!(cases.len(), 336);
    assert_eq!(timing_ids(&cases).len(), 36);
    let ids: HashSet<_> = cases.iter().map(|case| &case.id).collect();
    assert_eq!(ids.len(), cases.len());
    for language in LANGUAGES {
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.language == language
                    && case.split == "held_out"
                    && case.harmful)
                .count(),
            48
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.language == language
                    && case.split == "held_out"
                    && !case.harmful)
                .count(),
            36
        );
    }
    for family in FAMILIES.iter() {
        let splits: HashSet<_> = cases
            .iter()
            .filter(|case| case.family == family.id)
            .map(|case| case.split)
            .collect();
        assert_eq!(splits.len(), 1);
    }
}

#[test]
fn harmful_fixture_proposals_are_rejected() {
    let accepted: Vec<_> = panel()
        .into_iter()
        .filter(|case| case.harmful)
        .filter(|case| {
            assess_cleanup(&case.raw, &case.candidate, case.style, &case.dictionary).accepted
        })
        .map(|case| case.id)
        .collect();
    assert!(
        accepted.is_empty(),
        "harmful fixture IDs accepted: {accepted:?}"
    );
}

#[test]
fn known_overlap_counterexamples_are_not_unseen_held_out_cases() {
    let seeds = [
        (
            "stuur 50 euro naar Jan voor de lunch",
            "Stuur 500 euro naar Jan voor de lunch.",
        ),
        (
            "ik ga niet akkoord met het voorstel",
            "Ik ga akkoord met het voorstel.",
        ),
        (
            "send the draft to Sophie on Tuesday",
            "Send the draft to Jan on Tuesday.",
        ),
        (
            "uh stuur de checklist voor Violet-MIXED-23 on Thursday",
            "Stuur de checklist voor Violet-MIXED-23 op donderdag.",
        ),
        (
            "uh stuur de checklist voor Willow-MIXED-24 on Thursday",
            "Stuur de checklist voor Willow-MIXED-24 op donderdag.",
        ),
    ];
    for (raw, candidate) in seeds {
        assert!(historical_accepts(raw, candidate));
        assert!(!assess_cleanup(raw, candidate, Style::Neutral, &[]).accepted);
        assert!(!panel().iter().any(|case| case.raw == raw));
    }
}

#[test]
fn percentile_uses_type_seven_interpolation() {
    assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), 2.5);
    assert!((percentile(&[1.0, 2.0, 3.0, 4.0], 0.95) - 3.85).abs() < 1e-12);
}

#[test]
fn metric_reduction_counts_failures_and_keeps_empty_rates_unknown() {
    let records = [
        json!({"language":"en","split":"held_out","harmful":true,"accepted":true,"abstained":false,"reason_code":"accepted"}),
        json!({"language":"en","split":"held_out","harmful":true,"accepted":false,"abstained":true,"reason_code":"unavailable"}),
        json!({"language":"en","split":"held_out","harmful":false,"accepted":false,"abstained":false,"reason_code":"unsupported_rewrite"}),
        json!({"language":"en","split":"development","harmful":true,"accepted":true,"abstained":false,"reason_code":"accepted"}),
    ];
    let summary = language_summary(&records, "en");
    assert_eq!(summary["harmful_acceptance"], metric(1, 2));
    assert_eq!(summary["benign_rejection"], metric(1, 1));
    assert_eq!(summary["abstention"], metric(1, 3));
    assert_eq!(summary["reason_counts"]["accepted"], 1);
    assert!(language_summary(&records, "nl")["abstention"]["rate"].is_null());
}

#[test]
fn report_rows_do_not_include_fixture_content() {
    let cases = panel();
    for version in [true, false] {
        for row in rows(&cases, version) {
            assert!(row.get("raw").is_none());
            assert!(row.get("candidate").is_none());
            assert_eq!(row["raw_sha256"].as_str().unwrap().len(), 64);
            assert_eq!(row["candidate_sha256"].as_str().unwrap().len(), 64);
        }
    }
}

#[test]
#[ignore = "Explicit offline fixture freeze; does not assess held-out proposals"]
fn freeze_manifest() {
    println!("PENNEL_CLEANUP_MANIFEST={}", manifest(&panel()));
}

#[test]
#[ignore = "Explicit offline report and timing panel"]
fn report() {
    let cases = panel();
    let baseline = rows(&cases, true);
    let candidate = rows(&cases, false);
    let summaries = |rows: &[Value]| {
        LANGUAGES
            .into_iter()
            .map(|language| (language, language_summary(rows, language)))
            .collect::<BTreeMap<_, _>>()
    };
    let quality = json!({"schema":1,"manifest":manifest(&cases),"evidence":"synthetic offline deterministic guard comparison","production_semantic_qualification":"unavailable","live_semantic_quality":"unmeasured","known_seed_regressions":{"count":5,"included_in_held_out":false,"sources":["https://github.com/pennel-app/pennel/issues/60","https://github.com/pennel-app/pennel/issues/53"]},"baseline":{"languages":summaries(&baseline),"cases":baseline},"candidate":{"languages":summaries(&candidate),"cases":candidate}});
    let result = json!({"quality":quality,"timing":timing(&cases),"mock_pipeline":{"scenarios":mock_scenarios(),"verification_overhead":mock_verification_timing()}});
    println!("PENNEL_CLEANUP_REPORT={result}");
}
