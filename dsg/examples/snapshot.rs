use dsg::Dsg;

fn main() {
    let mut g = Dsg::new();

    // Example: build two layers, two nodes, one edge.
    assert!(g.add_layer(1));
    assert!(g.add_layer(2));

    assert!(g.add_node(1, 1001, [0.0, 0.0, 0.0]));
    assert!(g.set_label(1, 1001, "place_0"));

    assert!(g.add_node(2, 2001, [1.2, 0.5, 0.3]));
    assert!(g.set_label(2, 2001, "mug"));

    assert!(g.add_edge(1001, 2001));

    assert!(g.save_json("graph.json"));
    println!("Wrote graph.json");
}
