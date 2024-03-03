pub trait Symbol {
    fn name(&self) -> &str;
    fn display_name(&self) -> &str;
}
