pub mod toggle;
pub mod slider;
pub mod number_input;
pub mod tag_input;
pub mod password_input;
pub mod section_card;
pub mod select_group;
pub mod multi_checkbox;

/// A selectable option for toggle groups and checkboxes.
#[derive(Clone, PartialEq)]
pub struct SelectOption {
    pub value: &'static str,
    pub label: &'static str,
}