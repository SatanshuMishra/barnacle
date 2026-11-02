use crate::ids::RoleId;

pub fn may_organise(
    is_administrator: bool,
    member_roles: &[RoleId],
    organiser_roles: &[RoleId],
) -> bool {
    is_administrator
        || member_roles
            .iter()
            .any(|role| organiser_roles.contains(role))
}
