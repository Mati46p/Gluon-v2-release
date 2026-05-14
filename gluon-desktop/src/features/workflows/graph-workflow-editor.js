// Graph Workflow Editor - Visual graph editor for workflows
console.log("[GRAPH-EDITOR] GraphWorkflowEditor initializing...");

class GraphWorkflowEditor {
  constructor(containerId) {
    this.containerId = containerId;
    this.cy = null;
    this.initialized = false;
    this.nodeCounter = 0;
  }

  async init() {
    if (this.initialized) {
      console.log("[GRAPH-EDITOR] Already initialized");
      return;
    }

    try {
      // Check if Cytoscape is available
      if (typeof cytoscape === 'undefined') {
        console.warn("[GRAPH-EDITOR] Cytoscape library not loaded, graph editor disabled");
        return;
      }

      const container = document.getElementById(this.containerId);
      if (!container) {
        console.error("[GRAPH-EDITOR] Container not found:", this.containerId);
        return;
      }

      // Initialize Cytoscape
      this.cy = cytoscape({
        container: container,
        elements: [],
        style: [
          {
            selector: 'node',
            style: {
              'background-color': '#666',
              'label': 'data(label)',
              'text-valign': 'center',
              'text-halign': 'center',
              'width': '60px',
              'height': '60px'
            }
          },
          {
            selector: 'edge',
            style: {
              'width': 3,
              'line-color': '#ccc',
              'target-arrow-color': '#ccc',
              'target-arrow-shape': 'triangle',
              'curve-style': 'bezier'
            }
          }
        ],
        layout: {
          name: 'grid'
        }
      });

      this.initialized = true;
      console.log("[GRAPH-EDITOR] Initialized successfully");

      // Setup event listeners
      this.setupEventListeners();

    } catch (error) {
      console.error("[GRAPH-EDITOR] Initialization failed:", error);
    }
  }

  setupEventListeners() {
    if (!this.cy) return;

    // Node selection
    this.cy.on('tap', 'node', (evt) => {
      const node = evt.target;
      console.log("[GRAPH-EDITOR] Node selected:", node.data());
      this.onNodeSelected(node);
    });

    // Edge selection
    this.cy.on('tap', 'edge', (evt) => {
      const edge = evt.target;
      console.log("[GRAPH-EDITOR] Edge selected:", edge.data());
    });
  }

  onNodeSelected(node) {
    // Notify other components about node selection
    window.dispatchEvent(new CustomEvent('workflow-node-selected', {
      detail: { node: node.data() }
    }));
  }

  addNode(type, position) {
    if (!this.cy) {
      console.error("[GRAPH-EDITOR] Editor not initialized");
      return null;
    }

    const nodeId = `node-${++this.nodeCounter}`;
    const label = this.getNodeLabel(type);

    const node = {
      data: {
        id: nodeId,
        label: label,
        type: type
      },
      position: position || { x: 100, y: 100 }
    };

    this.cy.add(node);
    console.log("[GRAPH-EDITOR] Node added:", node);

    return nodeId;
  }

  // Handle drop event specifically to calculate correct coordinates
  addNodeAtEvent(type, event) {
    console.log("[GRAPH-EDITOR] addNodeAtEvent called for type:", type);

    if (!this.cy) {
        console.error("[GRAPH-EDITOR] Cytoscape instance (this.cy) is null!");
        return null;
    }

    // Get the DOM element position
    const container = this.cy.container();
    const rect = container.getBoundingClientRect();

    // Calculate position relative to the container
    const clientX = event.clientX;
    const clientY = event.clientY;

    const x = clientX - rect.left;
    const y = clientY - rect.top;

    console.log(`[GRAPH-EDITOR] Mouse: (${clientX}, ${clientY}) | Container Rect: left=${rect.left}, top=${rect.top} | Rel: (${x}, ${y})`);

    // Convert to model coordinates (taking pan/zoom into account)
    const pan = this.cy.pan();
    const zoom = this.cy.zoom();

    console.log(`[GRAPH-EDITOR] Pan: (${pan.x}, ${pan.y}) | Zoom: ${zoom}`);

    const modelX = (x - pan.x) / zoom;
    const modelY = (y - pan.y) / zoom;

    console.log(`[GRAPH-EDITOR] Calculated Model Pos: ({x: ${modelX}, y: ${modelY}})`);

    return this.addNode(type, { x: modelX, y: modelY });
  }

  getNodeLabel(type) {
    const labels = {
      'agent': '🤖 Agent',
      'report': '📊 Report',
      'autoapply': '⚡ Auto-Apply',
      'terminal': '💻 Terminal'
    };
    return labels[type] || type;
  }

  addEdge(sourceId, targetId) {
    if (!this.cy) {
      console.error("[GRAPH-EDITOR] Editor not initialized");
      return;
    }

    const edgeId = `edge-${sourceId}-${targetId}`;
    this.cy.add({
      data: {
        id: edgeId,
        source: sourceId,
        target: targetId
      }
    });

    console.log("[GRAPH-EDITOR] Edge added:", edgeId);
  }

  clear() {
    if (this.cy) {
      this.cy.elements().remove();
      this.nodeCounter = 0;
      console.log("[GRAPH-EDITOR] Graph cleared");
    }
  }

  layout() {
    if (this.cy) {
      this.cy.layout({ name: 'breadthfirst' }).run();
      console.log("[GRAPH-EDITOR] Layout applied");
    }
  }

  fit() {
    if (this.cy) {
      this.cy.fit();
      console.log("[GRAPH-EDITOR] View fitted");
    }
  }

  getGraphData() {
    if (!this.cy) return null;

    return {
      nodes: this.cy.nodes().map(n => n.data()),
      edges: this.cy.edges().map(e => e.data())
    };
  }

  loadGraphData(data) {
    if (!this.cy) return;

    this.clear();

    if (data.nodes) {
      data.nodes.forEach(nodeData => {
        this.cy.add({ data: nodeData });
      });
    }

    if (data.edges) {
      data.edges.forEach(edgeData => {
        this.cy.add({ data: edgeData });
      });
    }

    this.fit();
    console.log("[GRAPH-EDITOR] Graph data loaded");
  }
}

// Export to window
window.GraphWorkflowEditor = GraphWorkflowEditor;

console.log("[GRAPH-EDITOR] GraphWorkflowEditor script loaded successfully");