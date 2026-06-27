use super::{
    domain::Label,
    dto::{CreateLabelRequest, UpdateLabelRequest},
};

crate::crud_handlers! {
    module = super,
    entity = Label,
    create_req = CreateLabelRequest,
    update_req = UpdateLabelRequest,
    list_fn = list_labels,
    create_fn = create_label,
    update_fn = update_label,
    delete_fn = delete_label,
}
