use super::{
    domain::Tag,
    dto::{CreateTagRequest, UpdateTagRequest},
};

crate::crud_handlers! {
    module = super,
    entity = Tag,
    create_req = CreateTagRequest,
    update_req = UpdateTagRequest,
    list_fn = list_tags,
    create_fn = create_tag,
    update_fn = update_tag,
    delete_fn = delete_tag,
}
