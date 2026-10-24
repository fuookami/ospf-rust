use super::IterativeTaskCompilation;

struct RevisionEncoder(String);

impl RevisionEncoder {
    fn byte(&mut self, value: u8) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.0.push(HEX[(value >> 4) as usize] as char);
        self.0.push(HEX[(value & 15) as usize] as char);
    }

    fn index(&mut self, mut value: usize) {
        loop {
            let rest = value >> 7;
            self.byte((value & 127) as u8 | if rest == 0 { 0 } else { 128 });
            value = rest;
            if value == 0 { break; }
        }
    }

    fn number(&mut self, value: f64) {
        // 原 Debug 将所有 NaN 表示为同一状态，正负零仍须区分。 / Debug treated all NaNs as one state while signed zeros remain distinct.
        let bits = if value.is_nan() { f64::NAN.to_bits() } else { value.to_bits() };
        for byte in bits.to_le_bytes() { self.byte(byte); }
    }

    fn optional_number(&mut self, value: Option<f64>) {
        self.0.push(if value.is_some() { '1' } else { '0' });
        if let Some(value) = value { self.number(value); }
    }

    fn text(&mut self, value: &str) {
        self.index(value.len());
        for &byte in value.as_bytes() { self.byte(byte); }
    }

    fn indices(&mut self, values: &[usize]) {
        self.index(values.len());
        for &value in values { self.index(value); }
    }

    fn terms(&mut self, rows: &[Vec<(usize, f64)>]) {
        self.index(rows.len());
        for row in rows {
            self.index(row.len());
            for &(index, value) in row { self.index(index); self.number(value); }
        }
    }
}

pub(super) fn encode(compilation: &IterativeTaskCompilation) -> String {
    // 长度前缀和固定字段布局保留精确状态，不以普通哈希替代证据。 / Length prefixes and a fixed field layout preserve exact state without hashing evidence.
    let mut encoder = RevisionEncoder("task-state-v1:".into());
    encoder.index(compilation.columns.len());
    for column in &compilation.columns {
        for index in [column.index, column.task_index, column.executor_index, column.iteration, column.x_model_index] {
            encoder.index(index);
        }
        encoder.optional_number(column.cost.cost_sum);
        encoder.index(column.cost.items.len());
        for item in &column.cost.items {
            encoder.text(&item.tag);
            encoder.optional_number(item.cost_quantity);
            encoder.0.push(if item.message.is_some() { '1' } else { '0' });
            if let Some(message) = &item.message { encoder.text(message); }
        }
    }
    let mut removed = compilation.removed_columns.iter().copied().collect::<Vec<_>>();
    removed.sort_unstable();
    let mut fixed = compilation.fixed_columns.iter().copied().collect::<Vec<_>>();
    fixed.sort_unstable();
    encoder.indices(&removed);
    encoder.indices(&fixed);
    encoder.terms(&compilation.task_assignment_terms);
    encoder.terms(&compilation.task_compilation_terms);
    encoder.terms(&compilation.executor_compilation_terms);
    encoder.indices(&compilation.y_indices);
    encoder.indices(&compilation.z_indices);
    encoder.0.push(if compilation.task_cancel_enabled { '1' } else { '0' });
    encoder.0.push(if compilation.with_executor_leisure { '1' } else { '0' });
    encoder.0
}
