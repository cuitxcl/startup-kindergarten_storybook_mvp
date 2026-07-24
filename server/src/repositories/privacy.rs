pub(crate) fn storybook_privacy_risks(text: &str) -> Vec<&'static str> {
    let mut risks = Vec::new();
    if contains_email(text) {
        risks.push("邮箱");
    }
    if contains_chinese_mobile(text) {
        risks.push("手机号");
    }
    if contains_id_card(text) || contains_any(text, &["身份证", "身份证号", "证件号码"])
    {
        risks.push("身份信息");
    }
    if contains_any(text, &["家庭住址", "详细地址", "门牌号", "楼栋", "单元号"]) {
        risks.push("住址信息");
    }
    if contains_any(text, &["病历", "诊断证明", "医保卡", "过敏史", "就诊记录"]) {
        risks.push("医疗信息");
    }
    risks
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| text.contains(keyword))
}

fn contains_email(text: &str) -> bool {
    text.split(|ch: char| {
        !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-' | '@'))
    })
    .any(|candidate| {
        let Some((local, domain)) = candidate.split_once('@') else {
            return false;
        };
        !local.is_empty()
            && !domain.is_empty()
            && domain.contains('.')
            && local
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-'))
            && domain
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
    })
}

fn contains_chinese_mobile(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '1' && (index == 0 || !chars[index - 1].is_ascii_digit()) {
            let mut digits = String::new();
            let mut cursor = index;
            while cursor < chars.len() && digits.len() < 11 {
                let ch = chars[cursor];
                if ch.is_ascii_digit() {
                    digits.push(ch);
                } else if ch == ' ' || ch == '-' {
                    // Allow common formatting such as 138 0013 8000 or 138-0013-8000.
                } else {
                    break;
                }
                cursor += 1;
            }
            if digits.len() == 11
                && (cursor == chars.len() || !chars[cursor].is_ascii_digit())
                && matches!(digits.as_bytes()[1] as char, '3'..='9')
            {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn contains_id_card(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let value = token.trim_matches(|ch: char| {
            ch.is_ascii_punctuation() || "，。；、：（）《》【】“”‘’".contains(ch)
        });
        value.len() == 18
            && value
                .chars()
                .enumerate()
                .all(|(index, ch)| ch.is_ascii_digit() || (index == 17 && matches!(ch, 'x' | 'X')))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storybook_privacy_risks_allows_normal_storybook_text() {
        let risks = storybook_privacy_risks(
            "午睡小小约定 建立睡前整理和安静入睡流程 第 1 页 孩子们把鞋子摆整齐",
        );

        assert!(risks.is_empty());
    }

    #[test]
    fn storybook_privacy_risks_detects_contact_and_private_details() {
        let risks = storybook_privacy_risks(
            "请联系家长 parent@example.com 或 138 0013 8000，身份证号 11010119900307123X",
        );

        assert!(risks.contains(&"邮箱"));
        assert!(risks.contains(&"手机号"));
        assert!(risks.contains(&"身份信息"));
    }

    #[test]
    fn storybook_privacy_risks_detects_address_and_medical_text() {
        let risks = storybook_privacy_risks("家庭住址在某小区 3 号楼，孩子有过敏史。");

        assert!(risks.contains(&"住址信息"));
        assert!(risks.contains(&"医疗信息"));
    }

    #[test]
    fn storybook_privacy_risks_does_not_treat_long_ids_as_phone_numbers() {
        let risks = storybook_privacy_risks("UI Smoke 普通绘本 1784538853883");

        assert!(risks.is_empty());
    }
}
