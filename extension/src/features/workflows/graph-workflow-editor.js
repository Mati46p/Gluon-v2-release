// Graph-based Workflow Editor using Cytoscape.js
// Provides visual node-based editing for agent workflows

class GraphWorkflowEditor {
    constructor(containerId, workflowManager) {
        this.containerId = containerId;
        this.workflowManager = workflowManager;
        this.cy = null;
        this.selectedNode = null;
        this.shiftKeyPressed = false;
        this.firstNodeForConnection = null;
        this.init();
    }

    init() {
        console.log('[GraphEditor] Initializing graph editor');

        // Initialize Cytoscape (will be loaded from CDN)
        this.initCytoscape();

        // Event listeners
        this.setupEventListeners();
    }

    initCytoscape() {
        const container = document.getElementById(this.containerId);
        if (!container) {
            console.error('[GraphEditor] Container not found:', this.containerId);
            return;
        }

        // Wait for Cytoscape to be loaded from CDN
        if (typeof cytoscape === 'undefined') {
            console.warn('[GraphEditor] Cytoscape not loaded yet, retrying...');
            setTimeout(() => this.initCytoscape(), 500);
            return;
        }

        this.cy = cytoscape({
            container: container,

            style: [
                // Node styles
                {
                    selector: 'node',
                    style: {
                        'background-color': '#1e293b',
                        'border-width': 2,
                        'border-color': '#3b82f6',
                        'label': 'data(label)',
                        'text-valign': 'center',
                        'text-halign': 'center',
                        'color': '#e2e8f0',
                        'font-size': '12px',
                        'width': 80,
                        'height': 80,
                        'shape': 'roundrectangle',
                        'text-wrap': 'wrap',
                        'text-max-width': 70
                    }
                },
                // Report node style (aggregator - NOT a model!)
                {
                    selector: 'node[type="Report"]',
                    style: {
                        'background-color': '#0f172a',
                        'background-opacity': 0.6,
                        'border-color': '#fbbf24',
                        'border-width': 4,
                        'border-style': 'dashed',
                        'shape': 'hexagon',
                        'width': 100,
                        'height': 100,
                        'font-size': '11px',
                        'font-weight': 'bold',
                        'color': '#fbbf24'
                    }
                },
                // Auto-Apply node style (executor - NOT a model!)
                {
                    selector: 'node[type="AutoApply"]',
                    style: {
                        'background-color': '#064e3b',
                        'background-opacity': 0.8,
                        'border-color': '#10b981',
                        'border-width': 4,
                        'border-style': 'solid',
                        'shape': 'diamond',
                        'width': 110,
                        'height': 110,
                        'font-size': '11px',
                        'font-weight': 'bold',
                        'color': '#10b981'
                    }
                },
                // Terminal node style (sensor - NOT a model!)
                {
                    selector: 'node[type="Terminal"]',
                    style: {
                        'background-color': '#1e3a8a',
                        'background-opacity': 0.8,
                        'border-color': '#60a5fa',
                        'border-width': 4,
                        'border-style': 'dotted',
                        'shape': 'rectangle',
                        'width': 100,
                        'height': 80,
                        'font-size': '11px',
                        'font-weight': 'bold',
                        'color': '#60a5fa'
                    }
                },
                // Connected agent
                {
                    selector: 'node.status-connected',
                    style: {
                        'border-color': '#10b981'
                    }
                },
                // Waiting agent
                {
                    selector: 'node.status-waiting',
                    style: {
                        'border-color': '#f59e0b'
                    }
                },
                // Disconnected agent
                {
                    selector: 'node.status-disconnected',
                    style: {
                        'border-color': '#ef4444'
                    }
                },
                // Selected node
                {
                    selector: 'node:selected',
                    style: {
                        'border-color': '#8b5cf6',
                        'border-width': 4
                    }
                },
                // Connection source node (for Shift+click mode)
                {
                    selector: 'node.connection-source',
                    style: {
                        'border-color': '#8b5cf6',
                        'border-width': 5,
                        'background-color': '#2d1b4e'
                    }
                },
                // Edge styles
                {
                    selector: 'edge',
                    style: {
                        'width': 2,
                        'line-color': '#475569',
                        'target-arrow-color': '#475569',
                        'target-arrow-shape': 'triangle',
                        'curve-style': 'bezier',
                        'arrow-scale': 1.5
                    }
                },
                // Edge with template
                {
                    selector: 'edge[template]',
                    style: {
                        'line-color': '#3b82f6',
                        'target-arrow-color': '#3b82f6',
                        'line-style': 'dashed'
                    }
                },
                // Selected edge
                {
                    selector: 'edge:selected',
                    style: {
                        'line-color': '#8b5cf6',
                        'target-arrow-color': '#8b5cf6',
                        'width': 3
                    }
                }
            ],

            layout: {
                name: 'preset', // Use saved positions
                fit: true,
                padding: 30
            },

            // Interaction settings
            minZoom: 0.5,
            maxZoom: 2,
            wheelSensitivity: 0.2
        });

        console.log('[GraphEditor] Cytoscape initialized');
    }

    setupEventListeners() {
        if (!this.cy) return;

        // Track Shift key state
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Shift') {
                this.shiftKeyPressed = true;
                document.getElementById('workflowGraphView')?.classList.add('shift-mode');
                console.log('[GraphEditor] Shift key pressed - connection mode enabled');
            }
        });

        document.addEventListener('keyup', (e) => {
            if (e.key === 'Shift') {
                this.shiftKeyPressed = false;
                this.firstNodeForConnection = null;
                document.getElementById('workflowGraphView')?.classList.remove('shift-mode');
                console.log('[GraphEditor] Shift key released - connection mode disabled');
                // Reset visual feedback
                if (this.cy) {
                    this.cy.nodes().removeClass('connection-source');
                }
            }
        });

        // Node click with Shift key for connection
        this.cy.on('tap', 'node', (evt) => {
            const node = evt.target;

            if (this.shiftKeyPressed) {
                // Connection mode
                if (!this.firstNodeForConnection) {
                    // First click - select source node
                    this.firstNodeForConnection = node;
                    node.addClass('connection-source');
                    this.workflowManager.showSuccessMessage(`Selected "${node.data('label')}" as source. Click another node to connect.`);
                    console.log('[GraphEditor] First node selected for connection:', node.id());
                } else {
                    // Second click - create connection
                    const fromId = this.firstNodeForConnection.id();
                    const toId = node.id();

                    if (fromId === toId) {
                        this.workflowManager.showErrorMessage('Cannot connect agent to itself');
                    } else if (this.workflowManager.connectionExists(fromId, toId)) {
                        this.workflowManager.showErrorMessage('Connection already exists');
                    } else {
                        // Show connection modal
                        const fromName = this.firstNodeForConnection.data('label');
                        const toName = node.data('label');

                        document.getElementById('connectFromId').value = fromId;
                        document.getElementById('connectToId').value = toId;
                        document.getElementById('connectFromLabel').textContent = fromName;
                        document.getElementById('connectToLabel').textContent = toName;

                        this.workflowManager.showModal('connectAgentsModal');
                    }

                    // Reset connection mode
                    this.cy.nodes().removeClass('connection-source');
                    this.firstNodeForConnection = null;
                }
                return; // Don't do normal selection
            }
        });

        // Node selection (without Shift)
        this.cy.on('select', 'node', (evt) => {
            if (!this.shiftKeyPressed) {
                this.selectedNode = evt.target;
                console.log('[GraphEditor] Node selected:', this.selectedNode.id());
            }
        });

        this.cy.on('unselect', 'node', () => {
            if (!this.shiftKeyPressed) {
                this.selectedNode = null;
            }
        });

        // Tooltip on hover
        const tooltip = document.getElementById('graphTooltip');

        this.cy.on('mouseover', 'node', (evt) => {
            const node = evt.target;
            const nodeType = node.data('type');

            if (nodeType === 'Report') {
                tooltip.textContent = '🗂️ AGREGATOR - Kolektor odpowiedzi (NIE jest modelem AI)';
                tooltip.classList.add('show');

                // Position tooltip near cursor
                this.cy.container().addEventListener('mousemove', (e) => {
                    tooltip.style.left = (e.offsetX + 15) + 'px';
                    tooltip.style.top = (e.offsetY - 30) + 'px';
                });
            } else if (nodeType === 'AutoApply') {
                tooltip.textContent = '⚡ AUTO-APPLY - Automatyczny executor kodu (NIE jest modelem AI)';
                tooltip.classList.add('show');

                // Position tooltip near cursor
                this.cy.container().addEventListener('mousemove', (e) => {
                    tooltip.style.left = (e.offsetX + 15) + 'px';
                    tooltip.style.top = (e.offsetY - 30) + 'px';
                });
            } else if (nodeType === 'Terminal') {
                tooltip.textContent = '🖥️ TERMINAL - Sensor output terminala (NIE jest modelem AI)';
                tooltip.classList.add('show');

                // Position tooltip near cursor
                this.cy.container().addEventListener('mousemove', (e) => {
                    tooltip.style.left = (e.offsetX + 15) + 'px';
                    tooltip.style.top = (e.offsetY - 30) + 'px';
                });
            }
        });

        this.cy.on('mouseout', 'node', () => {
            tooltip.classList.remove('show');
        });

        // Node drag end - save position
        this.cy.on('dragfree', 'node', (evt) => {
            const node = evt.target;
            const position = node.position();
            console.log('[GraphEditor] Node dragged:', node.id(), position);

            // Save position to backend
            this.saveNodePosition(node.id(), position.x, position.y);
        });

        // Double-click to show details
        this.cy.on('dblclick', 'node', (evt) => {
            const nodeId = evt.target.id();
            this.showNodeDetails(nodeId);
        });

        // Right-click context menu (simple version)
        this.cy.on('cxttap', 'node', (evt) => {
            const nodeId = evt.target.id();
            this.showContextMenu(nodeId, evt.renderedPosition);
        });

        // Edge selection
        this.cy.on('select', 'edge', (evt) => {
            console.log('[GraphEditor] Edge selected:', evt.target.id());
        });
    }

    async saveNodePosition(nodeId, x, y) {
        try {
            await this.workflowManager.sendWorkflowMessage('workflow_update_agent_position', {
                agent_id: nodeId,
                position: [x, y]
            });
        } catch (error) {
            console.error('[GraphEditor] Failed to save position:', error);
        }
    }

    showNodeDetails(nodeId) {
        // Find agent in graph data
        const graph = this.workflowManager.graph;
        if (!graph || !graph.agents) return;

        const agent = graph.agents[nodeId];
        if (!agent) return;

        console.log('[GraphEditor] Show details for:', agent.name);
        // TODO: Show modal with agent details
    }

    showContextMenu(nodeId, position) {
        console.log('[GraphEditor] Context menu for:', nodeId, 'at', position);
        // TODO: Implement context menu (Delete, Edit, Connect)
    }

    /**
     * Renders the workflow graph
     * @param {Object} graphData - Workflow graph data from backend
     */
     renderGraph(graphData) {
         if (!this.cy) {
             console.error('[GraphEditor] Cytoscape not initialized');
             return;
         }

         if (!graphData || !graphData.agents) {
             this.cy.elements().remove();
             return;
         }

         // --- SMART UPDATE (DIFFING) ---
         // Instead of clearing everything, update existing nodes

         const currentNodes = this.cy.nodes();
         const newAgents = graphData.agents || {};
         const newConnections = graphData.connections || [];

         // 1. Remove nodes not in new data
         currentNodes.forEach(node => {
             if (!newAgents[node.id()]) {
                 this.cy.remove(node);
             }
         });

         // 2. Add or Update nodes
         Object.values(newAgents).forEach((agent, index) => {
             let node = this.cy.getElementById(agent.id);

             const position = agent.position
                 ? { x: agent.position[0], y: agent.position[1] }
                 : null;

             const classes = `status-${agent.status.toLowerCase()}`;

             if (node.length === 0) {
                 // New Node
                 this.cy.add({
                     data: {
                         id: agent.id,
                         label: agent.name,
                         type: agent.agent_type || 'Normal',
                         status: agent.status,
                         pairingCode: agent.pairing_code,
                         wrapper: agent.output_wrapper
                     },
                     position: position || this.calculateDefaultPosition(index, Object.keys(newAgents).length),
                     classes: classes
                 });
             } else {
                 // Update Existing
                 node.data('label', agent.name);
                 node.data('status', agent.status);
                 // Update position only if provided and different (to allow dragging)
                 // if (position) node.position(position); 
                 node.classes(classes);
             }
         });

         // 3. Handle Edges (Simple removal and re-add is usually fine for edges)
         this.cy.edges().remove();

         const edges = newConnections.map((conn, index) => ({
             data: {
                 id: `edge-${conn.from_agent_id}-${conn.to_agent_id}`,
                 source: conn.from_agent_id,
                 target: conn.to_agent_id,
                 template: conn.message_template
             }
         }));
         this.cy.add(edges);

         // Auto layout only for fresh graphs
         if (currentNodes.length === 0 && Object.keys(newAgents).length > 0) {
              const hasPositions = Object.values(newAgents).some(a => a.position);
              if (!hasPositions) {
                  this.applyAutoLayout();
              } else {
                  this.cy.fit(30);
              }
         }
     }

    /**
     * Calculate default position for nodes without saved positions
     */
    calculateDefaultPosition(index, total) {
        const radius = 150;
        const angle = (index / total) * 2 * Math.PI;

        return {
            x: 300 + radius * Math.cos(angle),
            y: 250 + radius * Math.sin(angle)
        };
    }

    /**
     * Apply automatic layout
     */
    applyAutoLayout() {
        if (!this.cy) return;

        const layout = this.cy.layout({
            name: 'cose',
            animate: true,
            animationDuration: 500,
            nodeRepulsion: 8000,
            idealEdgeLength: 100,
            edgeElasticity: 100,
            nestingFactor: 1.2,
            gravity: 1,
            numIter: 1000,
            initialTemp: 200,
            coolingFactor: 0.95,
            minTemp: 1.0
        });

        layout.run();
    }

    /**
     * Get selected node
     */
    getSelectedNode() {
        return this.selectedNode;
    }

    /**
     * Clear selection
     */
    clearSelection() {
        if (this.cy) {
            this.cy.elements().unselect();
        }
        this.selectedNode = null;
    }

    /**
     * Add new node to graph
     */
    addNode(agent) {
        if (!this.cy) return;

        const position = agent.position
            ? { x: agent.position[0], y: agent.position[1] }
            : { x: 300, y: 250 }; // Center

        this.cy.add({
            data: {
                id: agent.id,
                label: agent.name,
                type: agent.agent_type || 'Normal',
                status: agent.status,
                pairingCode: agent.pairing_code,
                wrapper: agent.output_wrapper
            },
            position: position,
            classes: `status-${agent.status.toLowerCase()}`
        });
    }

    /**
     * Remove node from graph
     */
    removeNode(nodeId) {
        if (!this.cy) return;
        this.cy.getElementById(nodeId).remove();
    }

    /**
     * Add edge between nodes
     */
    addEdge(fromId, toId, template) {
        if (!this.cy) return;

        const edgeId = `edge-${fromId}-${toId}`;

        this.cy.add({
            data: {
                id: edgeId,
                source: fromId,
                target: toId,
                template: template
            }
        });
    }

    /**
     * Remove edge
     */
    removeEdge(fromId, toId) {
        if (!this.cy) return;

        const edge = this.cy.edges().filter(e =>
            e.data('source') === fromId && e.data('target') === toId
        );

        edge.remove();
    }

    /**
     * Destroy graph editor
     */
    destroy() {
        if (this.cy) {
            this.cy.destroy();
            this.cy = null;
        }
    }
}

// Export for use in workflow-manager.js
export { GraphWorkflowEditor };

// CommonJS fallback
if (typeof module !== 'undefined' && module.exports) {
    module.exports = GraphWorkflowEditor;
}