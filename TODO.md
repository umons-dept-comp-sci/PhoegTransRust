- [ ] Move everything to the new repo
- [X] Fix this bug with the neo4j output being different from the rust output
- [ ] Add different algorithm to select the next graphs
- [X] Add different similarity
- [X] Update the command line
- [ ] Update the documentation

From the "automaton" obtained with next and start, generate the underlying
undirected graph by adding an edge (u,v) if there is an arc (u,v) and an arc
(v,u).

Generate the maximal cliques in this graph and contract them. For each clique,
store the set of nodes. For each arc incident to the clique, store the set of
nodes of the clique incident to the edge (or just one ?).

When generating, consider i the node inputing into the clique and o the node
outputting. Generate each subset of the nodes in the clique that contains a node
incident to i and another to o. Special case if i and o are incident to the same
node. Also, consider that the two nodes will be adjacent and thus, there is only
one path of length 1.

Limitations:
- It is not possible in souffle's cpp interface to create new relations programmatically. Once the file has been compiled, it is final.
