use std::env;

#[tokio::main]
async fn main() {
    let pid: u32 = env::args()
        .nth(1)
        .expect("pid required")
        .parse()
        .expect("pid must be integer");

    let elements = lxs_state::atspi::walk_tree(pid).await.unwrap();
    for element in elements {
        println!(
            "{} role={} name={:?} actions={:?} bounds={:?}",
            element.index, element.role, element.name, element.actions, element.bounds
        );
    }
}
