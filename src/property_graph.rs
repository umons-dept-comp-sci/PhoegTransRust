use std::{collections::{HashMap, HashSet, VecDeque}, fmt::Display, ops::AddAssign};
use std::hash::Hash;

use petgraph::{algo::is_isomorphic_matching, graph::{DiGraph, EdgeIndex, NodeIndex}, stable_graph::StableDiGraph};
use thiserror::Error;

type Label = String;
type LabelId = u32;

type IsomorphismData<'a> = Option<(&'a HashMap<String, String>, HashSet<String>)>;

#[derive(Debug, Clone)]
struct IdManager<A>
where
    A: Copy + Default + AddAssign<A> + From<u8>,
{
    max_id: A,
    free_ids: VecDeque<A>,
}

impl<A> IdManager<A>
where
    A: Copy + Default + AddAssign<A> + From<u8>,
{
    fn get_id(&mut self) -> A {
        self.free_ids.pop_back().unwrap_or_else(|| {
            let id = self.max_id;
            self.max_id += 1.into();
            id
        })
    }

    fn free_id(&mut self, id: A) {
        self.free_ids.push_back(id);
    }
}

impl<A> Default for IdManager<A>
where
    A: Copy + Default + AddAssign<A> + From<u8>,
{
    fn default() -> Self {
        Self {
            max_id: Default::default(),
            free_ids: Default::default(),
        }
    }
}

#[derive(Error, Debug)]
pub enum LabelError {
    #[error("Unknown label id: {0}")]
    UnknownLabelId(LabelId),
}

#[derive(Debug, Clone)]
pub struct LabelMap<E>
where
    E: Hash + Eq + Copy,
{
    id_manager: IdManager<LabelId>,
    labels: HashMap<LabelId, Label>,
    label_ids: HashMap<Label, LabelId>,
    labels_map: HashMap<E, HashSet<LabelId>>,
    elements: HashMap<LabelId, HashSet<E>>,
}

impl<E> Default for LabelMap<E>
where
    E: Hash + Eq + Copy,
{
    fn default() -> Self {
        Self {
            id_manager: Default::default(),
            labels: Default::default(),
            label_ids: Default::default(),
            labels_map: Default::default(),
            elements: Default::default(),
        }
    }
}

impl<E> LabelMap<E>
where
    E: Hash + Eq + Copy,
{
    pub fn get_label(&self, id: LabelId) -> Option<&Label> {
        self.labels.get(&id)
    }

    pub fn get_id(&self, label: &Label) -> Option<&LabelId> {
        self.label_ids.get(label)
    }

    pub fn labels(&self) -> impl Iterator<Item = &LabelId> {
        self.labels.keys()
    }

    pub fn has_label(&self, element: &E, label : LabelId) -> bool {
        self.labels_map
            .get(element)
            .and_then(|set| {
                Some(set.contains(&label))
            } ).unwrap_or(false)
    }

    pub fn element_labels(&self, element: &E) -> impl Iterator<Item = &LabelId> {
        self.labels_map
            .get(element)
            .into_iter()
            .flat_map(|v| v.iter())
    }

    pub fn label_elements(&self, labelid: LabelId) -> impl Iterator<Item = &E> {
        self.elements
            .get(&labelid)
            .into_iter()
            .flat_map(|v| v.iter())
    }

    pub fn add_label(&mut self, label: Label) -> LabelId {
        let id = *self
            .label_ids
            .entry(label.clone())
            .or_insert_with(|| self.id_manager.get_id());
        self.labels.insert(id, label);
        self.elements.entry(id).or_default();
        id
    }

    pub fn delete_label(&mut self, id: LabelId) -> Result<(), LabelError> {
        self.labels
            .remove(&id)
            .and_then(|label| {
                self.id_manager.free_id(id);
                self.label_ids.remove(&label)
            })
            .ok_or(LabelError::UnknownLabelId(id))?;
        self.elements.remove(&id).and_then(|list| {
            for ele in list {
                self.labels_map.get_mut(&ele).and_then(|set| {
                    set.remove(&id);
                    Some(())
                });
            }
            Some(())
        });
        Ok(())
    }

    pub fn change_label(&mut self, id: LabelId, new_label: Label) -> Result<(), LabelError> {
        if !self.labels.contains_key(&id) {
            return Err(LabelError::UnknownLabelId(id));
        }
        self.labels
            .insert(id, new_label.clone())
            .and_then(|old_label| self.label_ids.remove(&old_label));
        self.label_ids.insert(new_label, id);
        Ok(())
    }

    pub fn add_label_mapping(&mut self, element: &E, labelid: LabelId) -> Result<(), LabelError> {
        if !self.labels.contains_key(&labelid) {
            return Err(LabelError::UnknownLabelId(labelid));
        }
        self.labels_map.entry(*element).or_default().insert(labelid);
        self.elements.entry(labelid).or_default().insert(*element);
        Ok(())
    }

    pub fn remove_label_mapping(&mut self, element: &E, labelid: LabelId) -> Result<(), LabelError> {
        if !self.labels.contains_key(&labelid) {
            return Err(LabelError::UnknownLabelId(labelid));
        }
        self.labels_map.get_mut(&element).and_then(|set| {
            set.remove(&labelid);
            self.elements.get_mut(&labelid).and_then(|set| {
                set.remove(element);
                Some(())
            });
            Some(())
        });
        Ok(())
    }

    pub fn remove_element(&mut self, element: &E) {
        self.labels_map.remove(element).and_then(|set| {
            set.iter().for_each(|label| {
                self.elements.get_mut(label).and_then(|set| {
                    set.remove(element);
                    Some(())
                });
            });
            Some(())
        });
    }
}

#[derive(Debug, Clone)]
pub struct Properties {
   pub name : String,
   pub map : HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PropertyGraph {
    pub graph: StableDiGraph<Properties, Properties, u32>,
    pub vertex_label : LabelMap<NodeIndex>,
    pub edge_label : LabelMap<EdgeIndex>,
}

impl PropertyGraph {
    fn display_label_prop(&self, f: &mut std::fmt::Formatter<'_>, labels : &Vec<&String>, props : &Properties) -> std::fmt::Result {
        write!(f, "{}:", props.name)?;
        let mut start = true;
        for label in labels {
            if !start {
                write!(f, "& ")?;
            } else {
                start = false;
            }
            write!(f, "{} ", label)?;
        }
        write!(f, "{{ ")?;
        start = true;
        for (key, typ) in props.map.iter() {
            if start {
                start = false; 
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{} {} ", key, typ)?;
        }
        write!(f, "}}")
    }

    fn build_isomorphic_input(&self) -> DiGraph<IsomorphismData, IsomorphismData> {
        let mut graph: DiGraph<IsomorphismData, IsomorphismData> = DiGraph::with_capacity(self.graph.node_count(), 0);
        let mut vertex_map = HashMap::new();
        self.graph.node_indices().for_each(|index| {
            let labels: HashSet<String> = self.vertex_label.element_labels(&index).map(|label| self.vertex_label.get_label(*label).unwrap().clone()).collect();
            let props = self.graph.node_weight(index).unwrap();
            vertex_map.insert(index, graph.add_node(Some((&props.map, labels))));
        });
        self.graph.edge_indices().for_each(|edge| {
            let (self_node_from, self_node_to) = self.graph.edge_endpoints(edge).unwrap();
            let graph_from = *vertex_map.get(&self_node_from).unwrap();
            let graph_to = *vertex_map.get(&self_node_to).unwrap();
            let labels: HashSet<String> = self.edge_label.element_labels(&edge).map(|label| self.edge_label.get_label(*label).unwrap().clone()).collect();
            let props = self.graph.edge_weight(edge).unwrap();
            let data = Some((&props.map, labels));
            if self.graph.edges_connecting(self_node_from, self_node_to).count() > 1 {
                let inter_node = graph.add_node(None);
                graph.add_edge(graph_from, inter_node, data);
                graph.add_edge(inter_node, graph_to, None);
            } else {
                graph.add_edge(graph_from, graph_to, data);
            }
        });
        graph
    }
    
    pub fn is_isomorphic(&self, other : &PropertyGraph) -> bool {
        let data_match = |data_left : &IsomorphismData, data_right: &IsomorphismData| {
            data_left == data_right
        };
        is_isomorphic_matching(&self.build_isomorphic_input(), &other.build_isomorphic_input(), data_match, data_match)
    }
}

impl Default for PropertyGraph {
    fn default() -> Self {
        Self { graph: Default::default(), vertex_label: Default::default(), edge_label: Default::default() }
    }
}

impl Display for PropertyGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "CREATE GRAPH TYPE {{")?;
        let mut names = HashMap::new();
        for vertex in self.graph.node_indices() {
            let props = self.graph.node_weight(vertex).unwrap();
            names.insert(vertex, props.name.clone());
            let labels = self.vertex_label.element_labels(&vertex).map(|id| self.vertex_label.get_label(*id).unwrap()).collect();
            write!(f, "( ")?;
            self.display_label_prop(f, &labels, props)?;
            writeln!(f, " )")?;
        }
        for edge in self.graph.edge_indices() {
            let (from, to) = self.graph.edge_endpoints(edge).unwrap();
            let props = self.graph.edge_weight(edge).unwrap();
            let labels = self.edge_label.element_labels(&edge).map(|id| self.edge_label.get_label(*id).unwrap()).collect();
            writeln!(f, "(:{})", names.get(&from).unwrap())?;
            write!(f, "  -[")?;
            self.display_label_prop(f, &labels, props)?;
            writeln!(f, " ]->")?;
            writeln!(f, "(:{})", names.get(&to).unwrap())?;
        }
        writeln!(f,"}}")
    }
}


#[cfg(test)]
mod test {
    use std::{collections::HashSet, iter::FromIterator};

    use crate::{parsing::PropertyGraphParser, property_graph::{IdManager, LabelMap}};


    #[test]
    fn test_id_manager() {
        let mut manager : IdManager<usize> = Default::default();
        assert_eq!(0, manager.get_id());
        assert_eq!(1, manager.get_id());
        assert_eq!(2, manager.get_id());
        manager.free_id(1);
        assert_eq!(1, manager.get_id());
        assert_eq!(3, manager.get_id());
        manager.free_id(1);
        manager.free_id(2);
        manager.free_id(0);
        manager.free_id(3);
        assert_eq!(3, manager.get_id());
        assert_eq!(0, manager.get_id());
        assert_eq!(2, manager.get_id());
        assert_eq!(1, manager.get_id());
    }

    #[test]
    fn create_delete_unasigned_labels() {
        let mut map : LabelMap<usize> = Default::default();
        let id = map.add_label("test1".to_string());
        assert_eq!(id, *map.get_id(&("test1".to_string())).unwrap());
        assert_eq!(None, map.get_id(&("test2".to_string())));
        assert_eq!("test1".to_string(), *map.get_label(id).unwrap());
        assert_eq!(None, map.get_label(id+1));
        assert!(map.delete_label(id+1).is_err());
        assert!(map.delete_label(id).is_ok());
        assert!(map.delete_label(id).is_err());
        assert_eq!(None, map.get_id(&("test1".to_string())));
        assert_eq!(None, map.get_label(id));
    }

    #[test]
    fn label_iterator_test() {
        let mut map : LabelMap<u32> = Default::default();
        let mut labels : HashSet<String> = HashSet::from_iter(["test1".to_string(), "test2".to_string(), "test3".to_string()].into_iter());
        let mut ids : HashSet<u32> = labels.iter().map(|label| {
            map.add_label(label.clone())
        }).collect();
        assert_eq!(3, ids.len());
        map.labels().for_each(|id| {
            assert!(ids.remove(id));
            assert!(labels.remove(map.get_label(*id).unwrap()))
        })
    }

    #[test]
    fn test_change_label() {
        let mut map : LabelMap<usize> = Default::default();
        let id = map.add_label("label".to_string());
        map.change_label(id, "new_label".to_string()).unwrap();
        assert_eq!(None, map.get_id(&"label".to_string()));
        assert_eq!(id, *map.get_id(&"new_label".to_string()).unwrap());
        assert_eq!("new_label".to_string(), *map.get_label(id).unwrap());
        assert!(map.change_label(id+1, "label".to_string()).is_err());
    }

    #[test]
    fn test_adding_removing_element_labels() {
        let mut map : LabelMap<usize> = Default::default();
        let id1 = map.add_label("label1".to_string());
        let id2 = map.add_label("label2".to_string());
        let id3 = map.add_label("label3".to_string());
        map.add_label_mapping(&0, id1).unwrap();
        map.add_label_mapping(&0, id3).unwrap();
        map.add_label_mapping(&1, id2).unwrap();
        map.add_label_mapping(&2, id1).unwrap();
        assert!(map.add_label_mapping(&2, id1+id2+id3+1).is_err());

        let lab0 : Vec<_> = map.element_labels(&0).collect();
        assert!((lab0[0] == &id1 || lab0[0] == &id3) && (lab0[1] == &id1 || lab0[1] == &id3) && lab0[0] != lab0[1] && lab0.len() == 2);
        let lab1 : Vec<_> = map.element_labels(&1).collect();
        assert!(lab1[0] == &id2 && lab1.len() == 1);
        let lab2 : Vec<_> = map.element_labels(&2).collect();
        assert!(lab2[0] == &id1 && lab2.len() == 1);
        assert!(map.element_labels(&4).next().is_none());

        let el1 : Vec<_> = map.label_elements(id1).collect();
        assert!((el1[0] == &0 || el1[0] == &2) && (el1[1] == &0 || el1[1] == &2) && el1[0] != el1[1] && el1.len() == 2);
        let el2 : Vec<_> = map.label_elements(id2).collect();
        assert!(el2[0] == &1 && el2.len() == 1);
        let el3 : Vec<_> = map.label_elements(id3).collect();
        assert!(el3[0] == &0 && el3.len() == 1);

        map.remove_label_mapping(&0, id3).unwrap();
        let lab0 : Vec<_> = map.element_labels(&0).collect();
        assert!(lab0[0] == &id1 && lab0.len() == 1);
        assert!(map.remove_label_mapping(&0, id3).is_ok());

        map.delete_label(id1).unwrap();
        assert!(map.label_elements(id1).next().is_none());
        assert!(map.element_labels(&0).next().is_none());
        assert!(map.element_labels(&2).next().is_none());
        assert!(map.element_labels(&1).next().is_some());

        assert!(map.remove_label_mapping(&0, id1).is_err());
    }

    #[test]
    fn test_remove_element() {
        let mut map : LabelMap<usize> = Default::default();
        let id1 = map.add_label("label1".to_string());
        let id2 = map.add_label("label2".to_string());
        map.add_label_mapping(&0, id1).unwrap();
        map.add_label_mapping(&0, id2).unwrap();

        map.remove_element(&0);
        assert!(map.element_labels(&0).next().is_none());
        assert!(map.label_elements(id1).next().is_none());
        assert!(map.label_elements(id2).next().is_none());
    }

    #[test]
    fn test_isomorphism() {
        let parser = PropertyGraphParser;
        let specs = "create graph type g1 {
            (n1 : L1),
            (n2 : L1 & L3),
            (n3 : L2),
            (:n1)-[e1 : E1]->(:n2),
            (:n1)-[e2 : E4]->(:n2),
            (:n2)-[e3 : E2 & E5]->(:n3)
        }
        create graph type g2 {
            (u3 : L2),
            (u1 : L1),
            (u2 : L1 & L3),
            (:u2)-[f3 : E2 & E5]->(:u3),
            (:u1)-[f2 : E4]->(:u2),
            (:u1)-[f1 : E1]->(:u2)
        }
        ";
        let graphs = parser.convert_text(specs);
        let first_graph = graphs.get(0).unwrap();
        let second_graph = graphs.get(1).unwrap();
        assert!(first_graph.is_isomorphic(second_graph));
        assert!(second_graph.is_isomorphic(first_graph));
    }
}