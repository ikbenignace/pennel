# Synthetic corpus provenance

`corpus.py:build_corpus()` is the source of truth for corpus version 1. It constructs exactly 90 fictional meeting groups: ten development, ten calibration and ten held-out meetings for each of English, Dutch and mixed language. The generator is deterministic, has no random seed or clock dependency, reads no user files and makes no inference requests.

All transcripts, notes, summaries, names and cleanup examples were authored for issue #53. They are not recordings, private content, observed provider mistakes or vendor predictions. Scenario domains are disjoint across splits. Language versions have different fictional identifiers and amounts; no cross-split translation counterpart exists. Label-order variants retain their originating group and split. Cross-meeting questions list only same-split candidate meetings.

Gold spans use Unicode code-point offsets within the named transcript segment, notes or summary. They also retain exact text, relevance and support/contradiction judgments. A whole chunk's timestamp is not an exact claim offset. Public search, Ask's whole-context path and Ask's retrieved-context path must remain separate measurements.

Decision records include an explicit `label_order` array. Serialize criteria in that order. Sorting JSON object keys before sending a request would erase the reordered-label experiment. `canonical_json` retains deterministic insertion order for that reason, and the explicit array also keeps label order visible to order-insensitive metadata hashes.

Nine long meetings cross both the 60,000-byte Ask boundary and the 40,000-character summary boundary. Their archive records are deliberately irrelevant synthetic length stressors. Controlled grammar, repeated task wording and synthetic distractors limit external validity. This corpus does not establish natural-conversation quality, acoustic speaker recognition, packaged-app performance, field acceptance or production acceptance.

The runner must freeze corpus bytes, protocol, model identity and configuration before examining held-out inference results. Fixture annotations are authored expectations; no held-out model output was used in constructing them.
