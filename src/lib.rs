use layout::adt::dag::NodeHandle;
use layout::backends::svg::SVGWriter;
use layout::core::base::Orientation;
use layout::core::geometry::Point;
use layout::core::style::LineStyleKind;
use layout::core::style::StyleAttr;
use layout::std_shapes::shapes::Arrow;
use layout::std_shapes::shapes::LineEndKind;
use layout::std_shapes::shapes::RecordDef;
use wasm_bindgen::prelude::wasm_bindgen;

use layout::std_shapes::shapes::Element;
use layout::topo::layout::VisualGraph;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn prefix_tree(data: &str) -> String {
    let mut root = None;

    for line in data.split_whitespace() {
        PrefixTree::insert(&mut root, &mut line.chars());
    }

    let mut graph = VisualGraph::new(Orientation::TopToBottom);

    if let Some(mut root) = root {
        root.sort();
        root.convert(0, &mut graph);
    }

    let mut svg = SVGWriter::new();
    graph.do_it(false, false, false, &mut svg);
    let mut svg = svg.finalize();
    let i = svg.find("<svg").unwrap();
    svg.replace_range(0..i, "");
    svg
}

#[derive(Debug, Default)]
struct PrefixTree {
    children: Vec<(char, Option<PrefixTree>)>,
}

impl PrefixTree {
    fn insert(root: &mut Option<Self>, iter: &mut impl Iterator<Item = char>) {
        let Some(char) = iter.next() else { return };

        let root = root.get_or_insert_default();

        for (c, child) in &mut root.children {
            if char != *c {
                continue;
            }

            return Self::insert(child, iter);
        }

        Self::insert(&mut root.children.push_mut((char, None)).1, iter);
    }

    fn sort(&mut self) {
        self.children.iter_mut().for_each(|(_, child)| {
            if let Some(child) = child {
                child.sort()
            }
        });

        self.children.sort_unstable_by_key(|(char, _)| *char);
    }

    fn convert(&self, depth: usize, graph: &mut VisualGraph) -> NodeHandle {
        let (cells, children) = self
            .children
            .iter()
            .map(|(c, child)| {
                let cell = RecordDef::Text(String::from(*c), Some(format!("{c}{depth}")));
                let node = child
                    .as_ref()
                    .map(|child| (*c, child.convert(depth + 1, graph)));
                (cell, node)
            })
            .collect::<(Vec<_>, Vec<_>)>();

        let node = Element::create(
            layout::std_shapes::shapes::ShapeKind::Record(RecordDef::Array(cells)),
            StyleAttr::simple(),
            Orientation::LeftToRight,
            Point::new((30 * self.children.len()) as f64, 30.),
        );

        let parent = graph.add_node(node);

        for (c, child) in children.into_iter().flatten() {
            let arrow = Arrow::new(
                LineEndKind::None,
                LineEndKind::Arrow,
                LineStyleKind::Normal,
                "",
                &StyleAttr::simple(),
                &Some(format!("{c}{depth}")),
                &None,
            );

            graph.add_edge(arrow, parent, child);
        }

        parent
    }
}
