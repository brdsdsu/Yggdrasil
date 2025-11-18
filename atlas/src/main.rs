use atlas::Server;
fn main() -> Result<(), Box<dyn std::error::Error>> {

    let mut srv = Server::new();

    let sg = srv.scene_graph()?.clone();

    srv.update(sg);
    Ok(())
}
