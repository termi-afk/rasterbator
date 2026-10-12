//! Exact port of `iTextSharp 1.0.4` `ByteBuffer.formatDouble`.
//!
//! The original Rasterbator writes its PDFs through iTextSharp 1.0.4.0, whose
//! float formatter quantizes output: values below 1 are printed with up to 5
//! decimals, values up to 32767 with 2 decimals, larger values as rounded
//! integers, always with trailing zeros removed. Replicating this keeps the
//! content streams of this port comparable to the original's output.

/// Format a number the way iTextSharp 1.0.4 does.
pub fn format_double(d: f64) -> String {
    if d.abs() < 1.5e-5 {
        return "0".to_string();
    }
    let negative = d < 0.0;
    let d = if negative { -d } else { d };

    if d < 1.0 {
        let d = d + 5e-6;
        if d >= 1.0 {
            return if negative { "-1" } else { "1" }.to_string();
        }
        // Five decimals, truncated, trailing zeros trimmed (but one digit kept).
        let num = (d * 100_000.0) as i32;
        let digits = format!("{num:05}");
        let trimmed = digits.trim_end_matches('0');
        let trimmed = if trimmed.is_empty() { "0" } else { trimmed };
        let mut s = String::with_capacity(8);
        if negative {
            s.push('-');
        }
        s.push_str("0.");
        s.push_str(trimmed);
        return s;
    }

    if d <= 32767.0 {
        // Two decimals, rounded, trailing zeros trimmed.
        let scaled = ((d + 0.005) * 100.0) as i32;
        let int_part = scaled / 100;
        let frac = scaled % 100;
        let mut s = int_part.to_string();
        if frac != 0 {
            s.push('.');
            s.push(char::from_digit((frac / 10) as u32, 10).unwrap());
            if frac % 10 != 0 {
                s.push(char::from_digit((frac % 10) as u32, 10).unwrap());
            }
        }
        if negative {
            s.insert(0, '-');
        }
        return s;
    }

    let rounded = (d + 0.5) as i64;
    if negative {
        format!("-{rounded}")
    } else {
        rounded.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::format_double;

    #[test]
    fn matches_observed_original_output() {
        // Values sampled from PDFs produced by the original binary.
        assert_eq!(format_double(841.889_76), "841.89");
        assert_eq!(format_double(851.558_35), "851.56");
        assert_eq!(format_double(-9.668_601), "-9.67");
        assert_eq!(format_double(0.733_333_35), "0.73333");
        assert_eq!(format_double(0.642_17), "0.64217");
        assert_eq!(format_double(851.7), "851.7");
        assert_eq!(format_double(0.5), "0.5");
        assert_eq!(format_double(0.0), "0");
        assert_eq!(format_double(-0.0), "0");
        assert_eq!(format_double(0.000_02), "0.00002");
        assert_eq!(format_double(1.0), "1");
        assert_eq!(format_double(-1.0), "-1");
        assert_eq!(format_double(595.275_6), "595.28");
    }
}
