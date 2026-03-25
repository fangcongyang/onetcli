use sqlformat::{format, FormatOptions, QueryParams};

/// SQL 美化：将 SQL 格式化为可读性更好的多行形式
///
/// # 参数
/// * `sql` - 待格式化的 SQL 语句
/// * `uppercase` - 是否将关键字大写（默认 false）
pub fn format_sql(sql: &str, uppercase: bool) -> String {
    let options = FormatOptions {
        uppercase: if uppercase { Some(true) } else { None },
        ..FormatOptions::default()
    };
    format(sql, &QueryParams::None, &options)
}

/// SQL 压缩：将 SQL 压缩为单行形式
pub fn compress_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sql() {
        let sql = "select id, name from users where id = 1";
        let formatted = format_sql(sql, false);
        let formatted_upper = formatted.to_uppercase();
        assert!(formatted_upper.contains("SELECT"));
        assert!(formatted_upper.contains("FROM"));
        assert!(formatted_upper.contains("WHERE"));
    }

    #[test]
    fn test_format_sql_uppercase() {
        let sql = "select id, name from users where id = 1";
        let formatted = format_sql(sql, true);
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("FROM"));
        assert!(formatted.contains("WHERE"));
    }

    #[test]
    fn test_compress_sql() {
        let sql = "SELECT\n  id,\n  name\nFROM\n  users\nWHERE\n  id = 1";
        let compressed = compress_sql(sql);
        assert_eq!(compressed, "SELECT id, name FROM users WHERE id = 1");
    }
}
