use std::cmp::Ordering;
use std::collections::HashMap;
use crate::heap::Heap;
#[derive(Debug)]
pub enum TreeNode{
    Leaf{
        character : char,
        frequency :usize,
    },
    Branch{
        frequency :usize,
        left_children :Box<TreeNode>,
        right_children :Box<TreeNode>,
    }
}


struct ComparableTreeNode(Box<TreeNode>);
impl ComparableTreeNode {
    fn frequency(&self) -> usize{
        match &*self.0{
            TreeNode::Branch{frequency, ..} | TreeNode::Leaf{frequency, ..} => *frequency,
        }
    }
    fn into_tree_node(self) -> Box<TreeNode>{
        self.0
    }
}
impl PartialEq for ComparableTreeNode{
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for ComparableTreeNode{}
impl PartialOrd for ComparableTreeNode{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ComparableTreeNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.frequency().cmp(&other.frequency())
    }
}
#[derive(Debug)]
pub struct HuffmanTree{
    pub root: Box<TreeNode>,
}
pub fn parse_huffman_tree(frequency_map : &HashMap<char, usize>) -> Option<HuffmanTree> {
    if frequency_map.is_empty() {
        return None;
    }
    let mut heap : Heap<ComparableTreeNode> = Heap::new(frequency_map.into_iter().map(|(&character, &frequency)|{
        ComparableTreeNode(Box::new(TreeNode::Leaf {character, frequency}))
    }));
    while heap.len() >= 2{
        let pop1 = heap.pop().unwrap();
        let pop2 = heap.pop().unwrap();
        let merged = ComparableTreeNode(Box::new(TreeNode::Branch {
            frequency: pop1.frequency() + pop2.frequency(),
            left_children : pop1.into_tree_node(),
            right_children: pop2.into_tree_node(),
        }));
        heap.push(merged);
    }
    let root = heap.pop().unwrap().into_tree_node();
    Some(HuffmanTree{ root })
}
