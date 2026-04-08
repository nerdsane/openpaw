use temper_wasm_sdk::prelude::*;

/// Representative JLPT N5 vocabulary for v1 import demonstration.
fn n5_vocab_items() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("食べる", "taberu", "to eat"),
        ("飲む", "nomu", "to drink"),
        ("行く", "iku", "to go"),
        ("来る", "kuru", "to come"),
        ("見る", "miru", "to see"),
        ("聞く", "kiku", "to hear/ask"),
        ("読む", "yomu", "to read"),
        ("書く", "kaku", "to write"),
        ("話す", "hanasu", "to speak"),
        ("買う", "kau", "to buy"),
        ("大きい", "ookii", "big"),
        ("小さい", "chiisai", "small"),
        ("新しい", "atarashii", "new"),
        ("古い", "furui", "old"),
        ("高い", "takai", "tall/expensive"),
        ("安い", "yasui", "cheap"),
        ("良い", "yoi", "good"),
        ("悪い", "warui", "bad"),
        ("多い", "ooi", "many"),
        ("少ない", "sukunai", "few"),
        ("学校", "gakkou", "school"),
        ("先生", "sensei", "teacher"),
        ("学生", "gakusei", "student"),
        ("友達", "tomodachi", "friend"),
        ("家族", "kazoku", "family"),
        ("父", "chichi", "father"),
        ("母", "haha", "mother"),
        ("兄", "ani", "older brother"),
        ("姉", "ane", "older sister"),
        ("弟", "otouto", "younger brother"),
        ("妹", "imouto", "younger sister"),
        ("水", "mizu", "water"),
        ("お茶", "ocha", "tea"),
        ("朝", "asa", "morning"),
        ("昼", "hiru", "noon"),
        ("夜", "yoru", "night"),
        ("今日", "kyou", "today"),
        ("明日", "ashita", "tomorrow"),
        ("昨日", "kinou", "yesterday"),
        ("時間", "jikan", "time"),
        ("電車", "densha", "train"),
        ("車", "kuruma", "car"),
        ("道", "michi", "road"),
        ("駅", "eki", "station"),
        ("病院", "byouin", "hospital"),
        ("銀行", "ginkou", "bank"),
        ("本", "hon", "book"),
        ("映画", "eiga", "movie"),
        ("音楽", "ongaku", "music"),
        ("天気", "tenki", "weather"),
    ]
}

/// Representative basic kanji for v1 import demonstration.
fn basic_kanji_items() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("一", "ichi", "one"),
        ("二", "ni", "two"),
        ("三", "san", "three"),
        ("四", "shi/yon", "four"),
        ("五", "go", "five"),
        ("六", "roku", "six"),
        ("七", "shichi/nana", "seven"),
        ("八", "hachi", "eight"),
        ("九", "ku/kyuu", "nine"),
        ("十", "juu", "ten"),
        ("日", "nichi/hi", "day/sun"),
        ("月", "getsu/tsuki", "month/moon"),
        ("火", "ka/hi", "fire"),
        ("水", "sui/mizu", "water"),
        ("木", "moku/ki", "tree"),
        ("金", "kin/kane", "gold/money"),
        ("土", "do/tsuchi", "earth"),
        ("人", "jin/hito", "person"),
        ("大", "dai/ookii", "big"),
        ("小", "shou/chiisai", "small"),
    ]
}

/// Representative N5 grammar points for v1 import demonstration.
fn n5_grammar_items() -> Vec<(&'static str, &'static str, u32)> {
    vec![
        ("です", "Polite copula (is/am/are)", 1),
        ("ます", "Polite verb ending", 2),
        ("は (topic)", "Topic marker particle", 3),
        ("が (subject)", "Subject marker particle", 4),
        ("を (object)", "Object marker particle", 5),
        ("に (target)", "Target/direction particle", 6),
        ("で (means)", "Means/location particle", 7),
        ("と (and/with)", "Conjunction/companion particle", 8),
        ("も (also)", "Inclusive particle", 9),
        ("から (from)", "Starting point particle", 10),
        ("まで (until)", "Endpoint particle", 11),
        ("て form", "Verb connective form", 12),
        ("ている", "Ongoing state/action", 13),
        ("たい", "Want to (desire)", 14),
        ("ない (negation)", "Negative form", 15),
    ]
}

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "import_knowledge_bank: starting");

        // --- Read KnowledgeBank entity fields ---
        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or(json!({}));

        let domain = fields
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("japanese")
            .to_string();

        let source_config_raw = fields
            .get("source_config")
            .ok_or("missing source_config on KnowledgeBank")?;

        let source_config: Vec<serde_json::Value> =
            if let Some(arr) = source_config_raw.as_array() {
                arr.clone()
            } else if let Some(s) = source_config_raw.as_str() {
                serde_json::from_str(s).map_err(|e| {
                    format!("failed to parse source_config string: {e}")
                })?
            } else {
                return Err(
                    "source_config is neither an array nor a JSON string".into(),
                );
            };

        // --- Config ---
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

        let tenant = &ctx.tenant;
        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id)
            .to_string();

        let odata_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            (
                "x-temper-principal-kind".to_string(),
                "agent".to_string(),
            ),
            (
                "x-temper-principal-id".to_string(),
                ctx.entity_id.clone(),
            ),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        let mut total_concepts: u32 = 0;
        let mut total_links: u32 = 0;

        // Track created concept IDs for cross-referencing (kanji -> vocab links)
        let mut kanji_concept_ids: Vec<(String, String)> = Vec::new(); // (kanji_char, entity_id)
        let mut vocab_concept_ids: Vec<(String, String, String)> = Vec::new(); // (term, kanji_content, entity_id)

        for source in &source_config {
            let source_type = source
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            ctx.log("info", &format!(
                "import_knowledge_bank: processing source type '{source_type}'"
            ));

            match source_type {
                "structured_list" => {
                    let level = source
                        .get("level")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N5");
                    let difficulty_band = match level {
                        "N5" => "beginner",
                        "N4" => "elementary",
                        "N3" => "intermediate",
                        "N2" => "upper_intermediate",
                        "N1" => "advanced",
                        _ => "beginner",
                    };

                    for (term, reading, meaning) in n5_vocab_items() {
                        let concept_body = json!({
                            "fields": {
                                "domain": domain,
                                "concept_type": "vocab",
                                "label": term,
                                "content": json!({
                                    "term": term,
                                    "reading": reading,
                                    "meaning": meaning,
                                }),
                                "difficulty_band": difficulty_band,
                                "tags": [level, "vocab"],
                                "knowledge_bank_id": entity_id,
                            }
                        });

                        let create_resp = ctx.http_call(
                            "POST",
                            &format!("{temper_api_url}/tdata/Concepts"),
                            &odata_headers,
                            &concept_body.to_string(),
                        )?;
                        if create_resp.status < 200 || create_resp.status >= 300 {
                            return Err(format!("HTTP {}: {}", create_resp.status, &create_resp.body[..create_resp.body.len().min(300)]));
                        }

                        let created: serde_json::Value =
                            serde_json::from_str(&create_resp.body).map_err(|e| {
                                format!("failed to parse Concept creation for {term}: {e}")
                            })?;

                        let concept_entity_id = created
                            .get("entity_id")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                format!("created Concept for {term} has no entity_id")
                            })?
                            .to_string();

                        // Dispatch Define action
                        let define_url = format!(
                            "{temper_api_url}/tdata/EntitySets('{concept_entity_id}')/Temper.Define"
                        );
                        let define_resp = ctx.http_call(
                            "POST",
                            &define_url,
                            &odata_headers,
                            &json!({}).to_string(),
                        )?;
                        if define_resp.status < 200 || define_resp.status >= 300 {
                            return Err(format!("HTTP {}: {}", define_resp.status, &define_resp.body[..define_resp.body.len().min(300)]));
                        }

                        vocab_concept_ids.push((
                            term.to_string(),
                            term.to_string(),
                            concept_entity_id,
                        ));
                        total_concepts += 1;
                    }

                    ctx.log("info", &format!(
                        "import_knowledge_bank: created {} vocab concepts",
                        n5_vocab_items().len()
                    ));
                }

                "kanji_list" => {
                    for (kanji, reading, meaning) in basic_kanji_items() {
                        let concept_body = json!({
                            "fields": {
                                "domain": domain,
                                "concept_type": "kanji",
                                "label": kanji,
                                "content": json!({
                                    "character": kanji,
                                    "reading": reading,
                                    "meaning": meaning,
                                }),
                                "difficulty_band": "beginner",
                                "tags": ["kanji", "basic"],
                                "knowledge_bank_id": entity_id,
                            }
                        });

                        let create_resp = ctx.http_call(
                            "POST",
                            &format!("{temper_api_url}/tdata/Concepts"),
                            &odata_headers,
                            &concept_body.to_string(),
                        )?;
                        if create_resp.status < 200 || create_resp.status >= 300 {
                            return Err(format!("HTTP {}: {}", create_resp.status, &create_resp.body[..create_resp.body.len().min(300)]));
                        }

                        let created: serde_json::Value =
                            serde_json::from_str(&create_resp.body).map_err(|e| {
                                format!("failed to parse Concept creation for {kanji}: {e}")
                            })?;

                        let concept_entity_id = created
                            .get("entity_id")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                format!("created Concept for {kanji} has no entity_id")
                            })?
                            .to_string();

                        // Dispatch Define action
                        let define_url = format!(
                            "{temper_api_url}/tdata/EntitySets('{concept_entity_id}')/Temper.Define"
                        );
                        let define_resp = ctx.http_call(
                            "POST",
                            &define_url,
                            &odata_headers,
                            &json!({}).to_string(),
                        )?;
                        if define_resp.status < 200 || define_resp.status >= 300 {
                            return Err(format!("HTTP {}: {}", define_resp.status, &define_resp.body[..define_resp.body.len().min(300)]));
                        }

                        kanji_concept_ids
                            .push((kanji.to_string(), concept_entity_id));
                        total_concepts += 1;
                    }

                    ctx.log("info", &format!(
                        "import_knowledge_bank: created {} kanji concepts",
                        basic_kanji_items().len()
                    ));
                }

                "grammar_inventory" => {
                    let mut prev_concept_id: Option<String> = None;

                    for (pattern, description, order) in n5_grammar_items() {
                        let concept_body = json!({
                            "fields": {
                                "domain": domain,
                                "concept_type": "grammar",
                                "label": pattern,
                                "content": json!({
                                    "pattern": pattern,
                                    "description": description,
                                    "order": order,
                                }),
                                "difficulty_band": "beginner",
                                "tags": ["N5", "grammar"],
                                "knowledge_bank_id": entity_id,
                            }
                        });

                        let create_resp = ctx.http_call(
                            "POST",
                            &format!("{temper_api_url}/tdata/Concepts"),
                            &odata_headers,
                            &concept_body.to_string(),
                        )?;
                        if create_resp.status < 200 || create_resp.status >= 300 {
                            return Err(format!("HTTP {}: {}", create_resp.status, &create_resp.body[..create_resp.body.len().min(300)]));
                        }

                        let created: serde_json::Value =
                            serde_json::from_str(&create_resp.body).map_err(|e| {
                                format!("failed to parse Concept creation for {pattern}: {e}")
                            })?;

                        let concept_entity_id = created
                            .get("entity_id")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                format!("created Concept for {pattern} has no entity_id")
                            })?
                            .to_string();

                        // Dispatch Define action
                        let define_url = format!(
                            "{temper_api_url}/tdata/EntitySets('{concept_entity_id}')/Temper.Define"
                        );
                        let define_resp = ctx.http_call(
                            "POST",
                            &define_url,
                            &odata_headers,
                            &json!({}).to_string(),
                        )?;
                        if define_resp.status < 200 || define_resp.status >= 300 {
                            return Err(format!("HTTP {}: {}", define_resp.status, &define_resp.body[..define_resp.body.len().min(300)]));
                        }

                        // Create prerequisite link: each grammar point depends on the previous
                        if let Some(ref prev_id) = prev_concept_id {
                            let link_body = json!({
                                "fields": {
                                    "source_concept_id": prev_id,
                                    "target_concept_id": concept_entity_id,
                                    "link_type": "prerequisite",
                                    "strength": 0.8,
                                    "knowledge_bank_id": entity_id,
                                }
                            });
                            let link_resp = ctx.http_call(
                                "POST",
                                &format!("{temper_api_url}/tdata/ConceptLinks"),
                                &odata_headers,
                                &link_body.to_string(),
                            )?;
                            if link_resp.status < 200 || link_resp.status >= 300 {
                                return Err(format!("HTTP {}: {}", link_resp.status, &link_resp.body[..link_resp.body.len().min(300)]));
                            }
                            total_links += 1;
                        }

                        prev_concept_id = Some(concept_entity_id);
                        total_concepts += 1;
                    }

                    ctx.log("info", &format!(
                        "import_knowledge_bank: created {} grammar concepts",
                        n5_grammar_items().len()
                    ));
                }

                other => {
                    ctx.log("warn", &format!(
                        "import_knowledge_bank: unknown source type '{other}', skipping"
                    ));
                }
            }

            // Dispatch ImportProgress after each source
            let progress_url = format!(
                "{temper_api_url}/tdata/EntitySets('{entity_id}')/Temper.ImportProgress"
            );
            let progress_body = json!({
                "concepts_imported": total_concepts,
                "links_imported": total_links,
                "source_type": source_type,
            });
            let progress_resp = ctx.http_call(
                "POST",
                &progress_url,
                &odata_headers,
                &progress_body.to_string(),
            )?;
            if progress_resp.status < 200 || progress_resp.status >= 300 {
                return Err(format!("HTTP {}: {}", progress_resp.status, &progress_resp.body[..progress_resp.body.len().min(300)]));
            }
        }

        // --- Create kanji -> vocab prerequisite links ---
        // A vocab item is linked to a kanji if the vocab term contains that kanji character
        ctx.log("info", "import_knowledge_bank: creating kanji-vocab prerequisite links");

        for (kanji_char, kanji_eid) in &kanji_concept_ids {
            for (_, vocab_content, vocab_eid) in &vocab_concept_ids {
                if vocab_content.contains(kanji_char.as_str()) {
                    let link_body = json!({
                        "fields": {
                            "source_concept_id": kanji_eid,
                            "target_concept_id": vocab_eid,
                            "link_type": "prerequisite",
                            "strength": 0.9,
                            "knowledge_bank_id": entity_id,
                        }
                    });
                    let link_resp = ctx.http_call(
                        "POST",
                        &format!("{temper_api_url}/tdata/ConceptLinks"),
                        &odata_headers,
                        &link_body.to_string(),
                    )?;
                    if link_resp.status < 200 || link_resp.status >= 300 {
                        return Err(format!("HTTP {}: {}", link_resp.status, &link_resp.body[..link_resp.body.len().min(300)]));
                    }
                    total_links += 1;
                }
            }
        }

        // --- Dispatch ImportComplete on KnowledgeBank ---
        ctx.log("info", &format!(
            "import_knowledge_bank: dispatching ImportComplete — {} concepts, {} links",
            total_concepts, total_links
        ));

        let complete_url = format!(
            "{temper_api_url}/tdata/EntitySets('{entity_id}')/Temper.ImportComplete"
        );
        let complete_body = json!({
            "total_concepts": total_concepts,
            "total_links": total_links,
        });
        let complete_resp = ctx.http_call(
            "POST",
            &complete_url,
            &odata_headers,
            &complete_body.to_string(),
        )?;
        if complete_resp.status < 200 || complete_resp.status >= 300 {
            return Err(format!("HTTP {}: {}", complete_resp.status, &complete_resp.body[..complete_resp.body.len().min(300)]));
        }

        ctx.log("info", "import_knowledge_bank: completed successfully");

        set_success_result(
            "",
            &json!({
                "status": "ok",
                "total_concepts": total_concepts,
                "total_links": total_links,
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
