//! Speaker question detection helpers.

use crate::infra::storage::normalize_question_key;

pub(super) fn tail_chars(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_owned();
    }
    text.chars().skip(count - max_chars).collect()
}

pub(super) fn detect_questions(
    context_text: &str,
    new_text: &str,
    language: &str,
    previous_question: Option<&str>,
) -> Vec<String> {
    let normalized = normalize_question_candidate(context_text);
    if normalized.is_empty() {
        return Vec::new();
    }
    let current_text = normalize_question_candidate(new_text);
    let mut questions = question_mark_sentences(&current_text)
        .into_iter()
        .map(|question| expand_question_from_context(&question, &normalized))
        .collect::<Vec<_>>();
    if questions.is_empty() {
        let latest = last_sentence_candidate(new_text);
        if !latest.is_empty()
            && ends_with_sentence_terminal(&latest)
            && looks_like_question(&latest, language)
        {
            questions.push(latest);
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut previous = previous_question
        .map(normalize_question_candidate)
        .filter(|question| !question.is_empty());
    let mut output = Vec::new();
    for question in questions {
        let normalized = normalize_question_candidate(&question);
        let candidate = if is_short_question(&normalized) {
            previous
                .as_ref()
                .map(|value| normalize_question_candidate(&format!("{value} {normalized}")))
                .unwrap_or_else(|| normalized.clone())
        } else {
            normalized.clone()
        };
        previous = Some(candidate.clone());
        if seen.insert(normalize_question_key(&candidate)) {
            output.push(candidate);
        }
    }
    output
}

fn expand_question_from_context(question: &str, context_text: &str) -> String {
    let question = normalize_question_candidate(question);
    let question_key = question.to_lowercase();
    if question_key.is_empty() {
        return question;
    }
    question_mark_sentences(context_text)
        .into_iter()
        .rev()
        .find(|candidate| {
            let candidate_key = candidate.to_lowercase();
            candidate_key != question_key && candidate_key.ends_with(&question_key)
        })
        .unwrap_or(question)
}

fn question_mark_sentences(text: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if is_sentence_terminal(ch) {
            let end = index + ch.len_utf8();
            if ch == '?' || ch == '\u{ff1f}' {
                let question = normalize_question_candidate(&text[start..end]);
                if looks_like_question(&question, "") {
                    output.push(question);
                }
            }
            start = end;
        }
    }
    output
}

fn is_sentence_terminal(ch: char) -> bool {
    matches!(ch, '?' | '\u{ff1f}' | '.' | '!' | '\u{3002}' | '\u{ff01}')
}

fn ends_with_sentence_terminal(text: &str) -> bool {
    text.trim().chars().last().is_some_and(is_sentence_terminal)
}

pub(super) fn normalize_question_candidate(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .chars()
        .take(1000)
        .collect()
}

fn last_sentence_candidate(text: &str) -> String {
    let normalized = normalize_question_candidate(text);
    if normalized.is_empty() {
        return String::new();
    }
    let mut start = 0;
    for (index, ch) in normalized.char_indices() {
        if is_sentence_terminal(ch) {
            start = index + ch.len_utf8();
        }
    }
    if start >= normalized.len() {
        return normalized;
    }
    normalize_question_candidate(&normalized[start..])
}

fn looks_like_question(text: &str, language: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    if normalized.len() < 8 || normalized.len() > 1000 {
        return false;
    }
    normalized.contains('?')
        || normalized.contains('\u{ff1f}')
        || targeted_question_keyword(&normalized, language)
        || general_question_keyword(&normalized)
}

fn is_short_question(text: &str) -> bool {
    text.split_whitespace().count() <= 3
}

fn targeted_question_keyword(text: &str, language: &str) -> bool {
    let primary = language.split('-').next().unwrap_or(language);
    match language {
        "tr" => contains_any(text, TR_QUESTION_KEYWORDS),
        "en" | "en-US" | "en-AU" | "en-GB" | "en-IN" | "en-NZ" => {
            contains_any(text, EN_QUESTION_KEYWORDS)
        }
        "ar" | "ar-DZ" | "ar-TD" | "ar-EG" | "ar-IR" | "ar-IQ" | "ar-JO" | "ar-KW" | "ar-LB"
        | "ar-MA" | "ar-PS" | "ar-QA" | "ar-SA" | "ar-SD" | "ar-SY" | "ar-TN" | "ar-AE" => {
            contains_any(text, AR_QUESTION_KEYWORDS)
        }
        "zh" | "zh-CN" | "zh-Hans" | "zh-TW" | "zh-Hant" | "zh-HK" => {
            contains_any(text, ZH_QUESTION_KEYWORDS)
        }
        "ja" => contains_any(text, JA_QUESTION_KEYWORDS),
        "ko" | "ko-KR" => contains_any(text, KO_QUESTION_KEYWORDS),
        "he" => contains_any(text, HE_QUESTION_KEYWORDS),
        "hi" | "mr" => contains_any(text, HI_MR_QUESTION_KEYWORDS),
        "ur" => contains_any(text, UR_QUESTION_KEYWORDS),
        "fa" => contains_any(text, FA_QUESTION_KEYWORDS),
        "bn" => contains_any(text, BN_QUESTION_KEYWORDS),
        "ta" => contains_any(text, TA_QUESTION_KEYWORDS),
        "te" => contains_any(text, TE_QUESTION_KEYWORDS),
        "kn" => contains_any(text, KN_QUESTION_KEYWORDS),
        "th" | "th-TH" => contains_any(text, TH_QUESTION_KEYWORDS),
        _ if primary != language => targeted_question_keyword(text, primary),
        _ => false,
    }
}

fn general_question_keyword(text: &str) -> bool {
    contains_any(text, GENERAL_QUESTION_KEYWORDS)
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|needle| text.contains(needle))
}

const EN_QUESTION_KEYWORDS: &[&str] = &[
    "what ",
    "why ",
    "how ",
    "when ",
    "where ",
    "which ",
    "who ",
    "can you",
    "could you",
    "would you",
    "tell me about",
    "walk me through",
    "describe",
    "explain",
    "implement",
    "design",
    "solve",
];

const TR_QUESTION_KEYWORDS: &[&str] = &[
    "nedir",
    "neydi",
    "neden",
    "niye",
    "nasil",
    "nasil",
    "nasıl",
    "hangi",
    "hangisi",
    "kim",
    "kime",
    "kimi",
    "nerede",
    "nereden",
    "nereye",
    "ne zaman",
    "kac",
    "kaç",
    "kacinci",
    "kaçıncı",
    "kendinizden",
    "kendinden",
    "bahseder",
    "aciklar misin",
    "anlatir misin",
    "anlatır mısın",
    "aciklar misin",
    "açıklar mısın",
    "ornek verir misin",
    "örnek verir misin",
    "mi ",
    "mı ",
    "mu ",
    "mü ",
    "misin",
    "misiniz",
    "mısın",
    "musun",
    "musunuz",
    "müsün",
    "müsünüz",
];

const AR_QUESTION_KEYWORDS: &[&str] = &[
    "ماذا",
    "ما",
    "لماذا",
    "كيف",
    "متى",
    "أين",
    "اين",
    "أي",
    "اي",
    "هل",
    "من",
];

const ZH_QUESTION_KEYWORDS: &[&str] = &[
    "什么",
    "甚麼",
    "为什么",
    "為什麼",
    "怎么",
    "怎麼",
    "如何",
    "哪里",
    "哪裡",
    "何时",
    "何時",
    "谁",
    "誰",
    "哪",
    "吗",
    "嗎",
];

const JA_QUESTION_KEYWORDS: &[&str] = &[
    "何",
    "なぜ",
    "どう",
    "どこ",
    "いつ",
    "誰",
    "どれ",
    "ですか",
    "ますか",
    "でしょうか",
];

const KO_QUESTION_KEYWORDS: &[&str] = &[
    "무엇",
    "뭐",
    "왜",
    "어떻게",
    "어디",
    "언제",
    "누구",
    "어느",
    "인가요",
    "습니까",
    "까요",
];

const HE_QUESTION_KEYWORDS: &[&str] = &["מה", "למה", "איך", "איפה", "מתי", "מי", "האם", "איזה"];

const HI_MR_QUESTION_KEYWORDS: &[&str] = &[
    "क्या",
    "क्यों",
    "कैसे",
    "कहाँ",
    "कब",
    "कौन",
    "कौन सा",
    "कितना",
    "कितनी",
    "किस",
];

const UR_QUESTION_KEYWORDS: &[&str] = &[
    "کیا",
    "کیوں",
    "کیسے",
    "کہاں",
    "کب",
    "کون",
    "کون سا",
    "کتنا",
    "کس",
];

const FA_QUESTION_KEYWORDS: &[&str] = &[
    "چیست",
    "چه",
    "چرا",
    "چگونه",
    "کجا",
    "کی",
    "چه زمانی",
    "کدام",
    "آیا",
    "کیست",
];

const BN_QUESTION_KEYWORDS: &[&str] = &[
    "কি",
    "কী",
    "কেন",
    "কিভাবে",
    "কীভাবে",
    "কোথায়",
    "কোথায়",
    "কখন",
    "কে",
    "কোন",
];

const TA_QUESTION_KEYWORDS: &[&str] = &["என்ன", "ஏன்", "எப்படி", "எங்கே", "எப்போது", "யார்", "எது", "எந்த"];

const TE_QUESTION_KEYWORDS: &[&str] = &[
    "ఏమి",
    "ఏంటి",
    "ఎందుకు",
    "ఎలా",
    "ఎక్కడ",
    "ఎప్పుడు",
    "ఎవరు",
    "ఏది",
    "ఏ",
];

const KN_QUESTION_KEYWORDS: &[&str] = &["ಏನು", "ಏಕೆ", "ಹೇಗೆ", "ಎಲ್ಲಿ", "ಯಾವಾಗ", "ಯಾರು", "ಯಾವ"];

const TH_QUESTION_KEYWORDS: &[&str] = &[
    "อะไร",
    "ทำไม",
    "อย่างไร",
    "ยังไง",
    "ที่ไหน",
    "เมื่อไหร่",
    "ใคร",
    "ไหน",
    "หรือไม่",
    "ไหม",
];

const GENERAL_QUESTION_KEYWORDS: &[&str] = &[
    // Romance / Germanic / Slavic / Nordic question words.
    "que ",
    "quoi ",
    "pourquoi ",
    "comment ",
    "quand ",
    "quel ",
    "quelle ",
    "quels ",
    "quelles ",
    "combien ",
    "qué ",
    "por qué ",
    "porque ",
    "cómo ",
    "cuando ",
    "cuándo ",
    "donde ",
    "dónde ",
    "cuál ",
    "quién ",
    "cuánto ",
    "was ",
    "warum ",
    "wie ",
    "wann ",
    "wo ",
    "welche ",
    "wer ",
    "wieso ",
    "cosa ",
    "perche ",
    "perché ",
    "come ",
    "quando ",
    "dove ",
    "quale ",
    "chi ",
    "quanto ",
    "wat ",
    "waarom ",
    "hoe ",
    "wanneer ",
    "waar ",
    "welke ",
    "wie ",
    "o que ",
    "por que ",
    "como ",
    "onde ",
    "qual ",
    "quem ",
    "co ",
    "proc ",
    "proč ",
    "jak ",
    "kdy ",
    "kde ",
    "kdo ",
    "kolik ",
    "dlaczego ",
    "kiedy ",
    "gdzie ",
    "który ",
    "ktory ",
    "ile ",
    "czy ",
    "kaj ",
    "zakaj ",
    "kako ",
    "kdaj ",
    "kje ",
    "kateri ",
    "preco ",
    "prečo ",
    "ako ",
    "kedy ",
    "ktorý ",
    "ktory ",
    "koliko ",
    "ce ",
    "de ce ",
    "cum ",
    "cand ",
    "când ",
    "unde ",
    "care ",
    "cine ",
    "cat ",
    "cât ",
    "mit ",
    "miért ",
    "miert ",
    "hogyan ",
    "mikor ",
    "hol ",
    "melyik ",
    "mennyi ",
    "vad ",
    "varför ",
    "varfor ",
    "hur ",
    "när ",
    "nar ",
    "var ",
    "vilken ",
    "vem ",
    "hvad ",
    "hvorfor ",
    "hvordan ",
    "hvornår ",
    "hvornar ",
    "hvor ",
    "hvilken ",
    "hvem ",
    "hva ",
    "når ",
    "hvilken ",
    "mikä ",
    "mika ",
    "mitä ",
    "mita ",
    "miksi ",
    "miten ",
    "milloin ",
    "missä ",
    "missa ",
    "kuka ",
    "kuinka ",
    "mis ",
    "miks ",
    "kuidas ",
    "millal ",
    "kus ",
    "milline ",
    "kes ",
    "kas ",
    "kāpēc ",
    "kapec ",
    "kad ",
    "kur ",
    "cik ",
    "kodėl ",
    "kodel ",
    "kaip ",
    "kiek ",
    // Common interview/action prompts across languages.
    "expliquer ",
    "explique ",
    "décrire ",
    "describe ",
    "explica ",
    "explicar ",
    "describa ",
    "erklären ",
    "erklaren ",
    "beschreiben ",
    "spiega ",
    "descrivi ",
    "implemente ",
    "implementar ",
    "entwerfen ",
    "risolvere ",
    "resolver ",
    // Non-Latin question particles/words. These are substring checks so
    // question marks still remain the primary language-agnostic signal.
    "что",
    "почему",
    "как",
    "когда",
    "где",
    "какой",
    "кто",
    "сколько",
    "ли ",
    "що",
    "чому",
    "коли",
    "де",
    "який",
    "хто",
    "чи ",
    "τι ",
    "γιατί",
    "πως",
    "πώς",
    "πότε",
    "που",
    "πού",
    "ποιος",
    "πόσο",
    "ما",
    "ماذا",
    "لماذا",
    "كيف",
    "متى",
    "أين",
    "هل ",
    "من ",
    "מה",
    "למה",
    "איך",
    "איפה",
    "מתי",
    "מי ",
    "האם",
    "อะไร",
    "ทำไม",
    "อย่างไร",
    "ที่ไหน",
    "เมื่อไหร่",
    "ใคร",
    "ไหม",
    "何",
    "なぜ",
    "どう",
    "どこ",
    "いつ",
    "誰",
    "ですか",
    "ますか",
    "무엇",
    "뭐",
    "왜",
    "어떻게",
    "어디",
    "언제",
    "누구",
    "인가요",
    "습니까",
    "什么",
    "為什麼",
    "为什么",
    "怎么",
    "如何",
    "哪里",
    "哪裡",
    "何时",
    "誰",
    "谁",
    "吗",
    "嗎",
    "क्या",
    "क्यों",
    "कैसे",
    "कहाँ",
    "कब",
    "कौन",
    "कितना",
    "کیا",
    "کیوں",
    "کیسے",
    "کہاں",
    "کب",
    "کون",
    "چرا",
    "چگونه",
    "کجا",
    "کی",
    "چیست",
    "কি",
    "কেন",
    "কিভাবে",
    "কোথায়",
    "কখন",
    "কে",
    "என்ன",
    "ஏன்",
    "எப்படி",
    "எங்கே",
    "எப்போது",
    "யார்",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_question_mark_sentence_from_latest_text() {
        let questions = detect_questions(
            "We discussed the role. What tradeoffs would you consider?",
            "What tradeoffs would you consider?",
            "en",
            None,
        );

        assert_eq!(questions, vec!["What tradeoffs would you consider?"]);
    }

    #[test]
    fn detects_unmarked_interview_prompt_when_sentence_is_complete() {
        let questions = detect_questions(
            "Can you explain ownership.",
            "Can you explain ownership.",
            "en",
            None,
        );

        assert_eq!(questions, vec!["Can you explain ownership."]);
    }

    #[test]
    fn keeps_only_one_normalized_duplicate_question() {
        let questions =
            detect_questions("What is Rust?   What is Rust?", "What is Rust?", "en", None);

        assert_eq!(questions, vec!["What is Rust?"]);
    }
}
