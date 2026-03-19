use crate::domain::JobGrade;

/// Определяет грейд из названия вакансии
pub fn detect_grade(title: &str) -> Option<JobGrade> {
    let title_lower = title.to_lowercase();

    if title_lower.contains("intern") || title_lower.contains("стажер") {
        Some(JobGrade::Intern)
    } else if title_lower.contains("junior") || title_lower.contains("младший") {
        Some(JobGrade::Junior)
    } else if title_lower.contains("middle") {
        Some(JobGrade::Middle)
    } else if title_lower.contains("senior") || title_lower.contains("старший") {
        Some(JobGrade::Senior)
    } else if title_lower.contains("lead") || title_lower.contains("ведущий") {
        Some(JobGrade::Lead)
    } else if title_lower.contains("principal") {
        Some(JobGrade::Principal)
    } else if title_lower.contains("staff") {
        Some(JobGrade::Staff)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::JobGrade;
    use proptest::prelude::*;

    // 10.9 Property P4: определение грейда из заголовка
    // Feature: job-notifier-enhanced, Property 4: detect_grade возвращает правильный JobGrade для заголовков с ключевыми словами грейда
    proptest! {
        #[test]
        fn prop_p4_detect_grade_from_title(
            prefix in "[A-Za-z ]{0,10}",
            suffix in "[A-Za-z ]{0,10}",
            grade_idx in 0usize..14usize,
        ) {
            let grade_keywords = [
                ("junior", JobGrade::Junior),
                ("JUNIOR", JobGrade::Junior),
                ("senior", JobGrade::Senior),
                ("SENIOR", JobGrade::Senior),
                ("middle", JobGrade::Middle),
                ("MIDDLE", JobGrade::Middle),
                ("lead", JobGrade::Lead),
                ("LEAD", JobGrade::Lead),
                ("intern", JobGrade::Intern),
                ("INTERN", JobGrade::Intern),
                ("principal", JobGrade::Principal),
                ("PRINCIPAL", JobGrade::Principal),
                ("staff", JobGrade::Staff),
                ("STAFF", JobGrade::Staff),
            ];

            let (kw, expected_grade) = &grade_keywords[grade_idx];
            let title = format!("{} {} {}", prefix, kw, suffix);
            let result = detect_grade(&title);
            prop_assert_eq!(result.as_ref(), Some(expected_grade), "title: '{}'", title);
        }
    }

    #[test]
    fn test_detect_grade_no_keyword() {
        assert_eq!(detect_grade("Developer"), None);
    }

    #[test]
    fn test_detect_grade_russian() {
        assert_eq!(detect_grade("Младший разработчик"), Some(JobGrade::Junior));
        assert_eq!(detect_grade("Старший инженер"), Some(JobGrade::Senior));
        assert_eq!(detect_grade("Ведущий разработчик"), Some(JobGrade::Lead));
        assert_eq!(detect_grade("Стажер"), Some(JobGrade::Intern));
    }
}
