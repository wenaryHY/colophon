use super::{
    domain::Category,
    dto::{CreateCategoryRequest, UpdateCategoryRequest},
};

crate::crud_handlers! {
    module = super,
    entity = Category,
    create_req = CreateCategoryRequest,
    update_req = UpdateCategoryRequest,
    list_fn = list_categories,
    create_fn = create_category,
    update_fn = update_category,
    delete_fn = delete_category,
}
