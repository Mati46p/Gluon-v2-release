import { fileTreeLogger } from '../../common/logger.js';

// ============================================================================
// File Tree Management Module
// Zarządza projektami, drzewem plików i selekcją
// ============================================================================

import {
  fileTreeData, selectedNodes, selectedProjects, allProjects, searchQuery, searchTimeout,
  currentFileTreeRequestId, lastAction, collapsedNodes, BINARY_EXTENSIONS, PROJECT_COLORS,
  POLLING_INTERVAL_MS, fileTreePollingInterval, VIRTUAL_FILES_PROJECT_PATH,
  ragSelectedNodes, ragSelectedProjects,
  setFileTreeData, setSelectedNodes, setSearchQuery, setSearchTimeout, setCurrentFileTreeRequestId,
  setLastAction, setFileTreePollingInterval,
  showLoading, hideLoading, showStatusMessage, showError, formatSize, escapeHTML,
  saveSelectedProjects
} from './stateManagement.js';

// ============================================================================
// Project Management
// ============================================================================

/**
 * Wypełnia listę projektów
 */
export async function populateProjects(projects) {
  const container = document.getElementById('projectSelect');
  container.innerHTML = '';

  const validPaths = new Set(projects.map(p => p.path));
  const invalidPaths = [...selectedProjects].filter(path => !validPaths.has(path));

  if (invalidPaths.length > 0) {
    fileTreeLogger.log('Cleaning up invalid projects from cache:', invalidPaths);
    invalidPaths.forEach(path => selectedProjects.delete(path));
    await saveSelectedProjects();
    invalidPaths.forEach(path => selectedNodes.delete(path));
    updateSelectionInfo();
  }

  if (projects && projects.length > 0) {
    projects.forEach(project => {
      const projectName = project.path.split(/[\\/]/).pop() || project.path;
      const card = document.createElement('div');
      card.className = 'project-tab-card';
      card.dataset.path = project.path;

      if (selectedProjects.has(project.path)) {
        card.classList.add('active');
      }

      card.innerHTML = `<span class="project-tab-name" title="${project.path}">${projectName}</span><div class="project-tab-indicator"></div>`;
      card.addEventListener('click', () => handleProjectTabClick(card, project.path));
      container.appendChild(card);
    });
  } else {
    container.innerHTML = '<div class="empty-text" style="padding: 10px;">No projects. Add via desktop.</div>';
  }
}

/**
 * Obsługuje kliknięcie na kartę projektu
 */
export async function handleProjectTabClick(cardElement, projectPath) {
  if (selectedProjects.has(projectPath)) {
    selectedProjects.delete(projectPath);
  } else {
    selectedProjects.add(projectPath);
  }
  cardElement.classList.toggle('active');
  await saveSelectedProjects();

  // Load environment based on project selection
  await loadEnvironmentForSelectedProjects();

  triggerFileTreeLoadForSelected();
}

/**
 * Ładuje środowisko dla wybranych projektów
 * Jeśli wszystkie wybrane projekty mają to samo środowisko, ładuje je
 * W przeciwnym razie resetuje do domyślnego
 */
export async function loadEnvironmentForSelectedProjects() {
  if (selectedProjects.size === 0) {
    // No projects selected - reset to default
    fileTreeLogger.log('No projects selected, resetting to default');
    const { resetToDefaultEnvironment } = await import('./templatePromptManagement.js');
    await resetToDefaultEnvironment();
    return;
  }

  // Get environment_id for each selected project
  const environmentIds = [];
  for (const projectPath of selectedProjects) {
    const project = allProjects.find(p => p.path === projectPath);
    if (project && project.environmentId !== undefined && project.environmentId !== null) {
      environmentIds.push(project.environmentId);
    }
  }

  // Check if all projects have the same environment_id
  const uniqueEnvIds = [...new Set(environmentIds)];

  if (uniqueEnvIds.length === 1 && environmentIds.length === selectedProjects.size) {
    // All selected projects have the same environment - load it
    const projectPath = Array.from(selectedProjects)[0];
    fileTreeLogger.log(`All ${selectedProjects.size} selected project(s) have the same environment (ID: ${uniqueEnvIds[0]}), loading environment`);
    chrome.runtime.sendMessage({
      action: 'get_environment_for_project',
      payload: { path: projectPath }
    });
  } else {
    // Multiple environments or some projects without environment - reset to default
    fileTreeLogger.log('Multiple environments or missing assignments, resetting to default');
    const { resetToDefaultEnvironment } = await import('./templatePromptManagement.js');
    await resetToDefaultEnvironment();
  }
}

/**
 * Odznacza wszystkie projekty
 */
export async function handleDeselectAllProjects() {
  stopFileTreePolling();
  selectedProjects.clear();
  await saveSelectedProjects();
  document.querySelectorAll('.project-tab-card.active').forEach(card => card.classList.remove('active'));
  clearFileTree();
  selectedNodes.clear();
  updateSelectionInfo();
}

/**
 * Ładuje drzewo plików dla wybranych projektów
 */
export async function triggerFileTreeLoadForSelected() {
  clearFileTree();

  // Zachowaj zaznaczenia tylko dla projektów które są aktualnie wybrane
  // Usuń zaznaczenia dla projektów które zostały odznaczone
  const projectsToKeep = new Set(selectedProjects);
  for (const [projectPath] of selectedNodes) {
    if (!projectsToKeep.has(projectPath)) {
      selectedNodes.delete(projectPath);
    }
  }

  updateSelectionInfo();

  const paths = Array.from(selectedProjects);
  if (paths.length > 0) {
    showLoading(`Loading ${paths.length} project(s)...`, 'generateBtn');
    const requestId = await chrome.runtime.sendMessage({
      action: 'get_file_trees',
      payload: { paths: paths }
    });
    setCurrentFileTreeRequestId(requestId);
    startFileTreePolling();
  }
}

// ============================================================================
// File Tree Rendering
// ============================================================================

/**
 * Renders the RAG specific file tree into the modal
 */
export function renderRagFileTree() {
  const container = document.getElementById('ragFileTree');
  if (!container) return;

  container.innerHTML = '';

  // Use current global fileTreeData but filter by ragSelectedProjects
  const projectsToRender = fileTreeData.filter(p => ragSelectedProjects.has(p.projectPath));

  if (projectsToRender.length === 0) {
    container.innerHTML = `<div class="empty-state"><div class="empty-icon">📂</div><div class="empty-text">Select a project scope above</div></div>`;
    return;
  }

  projectsToRender.forEach((projectData, index) => {
    const projectName = projectData.projectPath.split(/[\\/]/).pop() || projectData.projectPath;
    const projectColor = PROJECT_COLORS[index % PROJECT_COLORS.length];

    const section = document.createElement('div');
    section.className = 'project-section';
    section.style.borderLeftColor = projectColor;

    // Use shared collapsedNodes state or handle locally? Let's use shared for consistency
    if (collapsedNodes.has(`rag::${projectData.projectPath}`)) {
      section.classList.add('collapsed');
    }

    const selectedCount = ragSelectedNodes.get(projectData.projectPath)?.size || 0;

    section.innerHTML = `
      <div class="project-header" title="${projectData.projectPath}">
        <span class="expand-icon">▼</span>
        <span class="project-name">${projectName}</span>
        <span class="project-stats">${projectData.fileCount} files, ${selectedCount} sel.</span>
        <div class="project-actions">
          <button class="rag-select-all" data-path="${projectData.projectPath}">All</button>
          <button class="rag-select-none" data-path="${projectData.projectPath}">None</button>
        </div>
      </div>
      <div class="project-tree-nodes"></div>`;

    // Header click (Collapse/Expand)
    section.querySelector('.project-header').addEventListener('click', (e) => {
      if (e.target.tagName === 'BUTTON') return;
      section.classList.toggle('collapsed');
      const key = `rag::${projectData.projectPath}`;
      if (section.classList.contains('collapsed')) collapsedNodes.add(key);
      else collapsedNodes.delete(key);
    });

    // Select All
    section.querySelector('.rag-select-all').addEventListener('click', () => {
      const allFiles = [];
      const traverse = (nodes) => {
        nodes.forEach(n => {
          if (n.nodeType === 'file') allFiles.push(n.path);
          if (n.children) traverse(n.children);
        });
      };
      traverse(projectData.tree);
      ragSelectedNodes.set(projectData.projectPath, new Set(allFiles));
      renderRagFileTree();
      updateRagSelectionInfo();
    });

    // Select None
    section.querySelector('.rag-select-none').addEventListener('click', () => {
      if (ragSelectedNodes.has(projectData.projectPath)) {
        ragSelectedNodes.get(projectData.projectPath).clear();
      }
      renderRagFileTree();
      updateRagSelectionInfo();
    });

    container.appendChild(section);
    renderRagNodeTree(projectData.tree || [], section.querySelector('.project-tree-nodes'), projectData.projectPath);
  });
}

function renderRagNodeTree(nodes, parentElement, projectPath) {
  nodes.forEach(node => {
    const nodeElement = document.createElement('div');
    nodeElement.className = `tree-node ${node.nodeType}`;

    const icon = node.nodeType === 'directory' ? '📁' : '📄';
    nodeElement.innerHTML = `<span class="tree-icon">${icon}</span><span class="tree-name">${node.name}</span>`;

    if (node.nodeType === 'file') {
      if (ragSelectedNodes.get(projectPath)?.has(node.path)) {
        nodeElement.classList.add('selected');
      }
      nodeElement.addEventListener('click', (e) => {
        e.stopPropagation();
        if (!ragSelectedNodes.has(projectPath)) ragSelectedNodes.set(projectPath, new Set());
        const set = ragSelectedNodes.get(projectPath);
        if (set.has(node.path)) set.delete(node.path);
        else set.add(node.path);

        nodeElement.classList.toggle('selected');
        updateRagSelectionInfo();
      });
    } else {
      // Directory
      nodeElement.addEventListener('click', (e) => {
        e.stopPropagation();
        // Toggle collapse
        const childrenContainer = nodeElement.nextElementSibling;
        if (childrenContainer) {
            nodeElement.classList.toggle('collapsed');
            childrenContainer.style.display = nodeElement.classList.contains('collapsed') ? 'none' : 'block';
        }
      });
    }

    parentElement.appendChild(nodeElement);

    if (node.children && node.children.length > 0) {
      const childrenContainer = document.createElement('div');
      childrenContainer.className = 'tree-children';
      parentElement.appendChild(childrenContainer);
      renderRagNodeTree(node.children, childrenContainer, projectPath);
    }
  });
}

export function updateRagSelectionInfo() {
  let totalFiles = 0;
  ragSelectedNodes.forEach(set => totalFiles += set.size);
  const countEl = document.getElementById('ragSelectionCount');
  if (countEl) countEl.textContent = `${totalFiles} files selected`;
}

/**
 * Renderuje scalone drzewo plików z wielu projektów
 */
export function renderMergedFileTree(projectsData) {
  const container = document.getElementById('fileTree');
  container.innerHTML = '';

  const virtualFiles = window.gluonVirtualFiles || new Map();
  const hasVirtualFiles = virtualFiles.size > 0;
  const hasProjects = Array.isArray(projectsData) && projectsData.length > 0;

  if (!hasProjects && !hasVirtualFiles) {
    return showEmptyState("Select a project or attach files");
  }

  // 1. Render Local Projects
  if (hasProjects) {
    projectsData.forEach((projectData, index) => {
      if (projectData.error) {
      const errorSection = createErrorSection(projectData);
      container.appendChild(errorSection);
      return;
    }

    const projectName = projectData.projectPath.split(/[\\/]/).pop() || projectData.projectPath;
    const projectColor = PROJECT_COLORS[index % PROJECT_COLORS.length];

    const section = document.createElement('div');
    section.className = 'project-section';
    section.style.borderLeftColor = projectColor;

    if (collapsedNodes.has(projectData.projectPath)) {
      section.classList.add('collapsed');
    }

    const selectedCount = selectedNodes.get(projectData.projectPath)?.size || 0;
    const statsText = selectedCount > 0
      ? `${projectData.fileCount} files, ${selectedCount} sel.`
      : `${projectData.fileCount} files`;

    section.innerHTML = `
      <div class="project-header" title="${projectData.projectPath}">
        <span class="expand-icon">▼</span>
        <span class="project-name">${projectName}</span>
        <span class="project-stats" id="stats-${btoa(projectData.projectPath)}">${statsText}</span>
        <div class="project-actions">
          <button title="Select all files in this project">All</button>
          <button title="Clear selection in this project">None</button>
        </div>
      </div>
      <div class="project-tree-nodes"></div>`;

    section.querySelector('.project-header').addEventListener('click', () => {
      section.classList.toggle('collapsed');
      if (section.classList.contains('collapsed')) {
        collapsedNodes.add(projectData.projectPath);
      } else {
        collapsedNodes.delete(projectData.projectPath);
      }
    });
    section.querySelector('.project-actions button:first-child').onclick = (e) => {
      e.stopPropagation();
      handleSelectAllInProject(projectData.projectPath, projectData.tree);
    };
    section.querySelector('.project-actions button:last-child').onclick = (e) => {
      e.stopPropagation();
      handleClearAllInProject(projectData.projectPath);
    };

    container.appendChild(section);
    renderNodeTree(projectData.tree || [], section.querySelector('.project-tree-nodes'), 0, projectData.projectPath);
  });
  }

  // 2. Render Virtual Files (Attached / Google Drive)
  if (hasVirtualFiles) {
    const section = document.createElement('div');
    section.className = 'project-section virtual-section';

    section.innerHTML = `
      <div class="project-header" title="Files attached from Google Drive or other sources">
        <span class="expand-icon">▼</span>
        <span class="project-name">☁️ Attached Files</span>
        <span class="project-stats">${virtualFiles.size} files</span>
      </div>
      <div class="project-tree-nodes"></div>`;

    const nodesContainer = section.querySelector('.project-tree-nodes');

    section.querySelector('.project-header').addEventListener('click', () => {
      section.classList.toggle('collapsed');
    });

    virtualFiles.forEach((content, filename) => {
      const node = document.createElement('div');
      node.className = 'tree-node file virtual-file';

      // Check selection state using the virtual project path
      if (selectedNodes.get(VIRTUAL_FILES_PROJECT_PATH)?.has(filename)) {
        node.classList.add('selected');
      }

      node.innerHTML = `<span class="tree-icon">📎</span><span class="tree-name">${filename}</span>`;
      node.title = `${filename} (Attached)`;

      // Add click listener for selection
      node.addEventListener('click', (event) => {
        event.stopPropagation();
        // Create a mock node object for handleNodeClick
        const mockNode = { path: filename, nodeType: 'file', name: filename };
        handleNodeClick(mockNode, node, VIRTUAL_FILES_PROJECT_PATH);
      });

      nodesContainer.appendChild(node);
    });

    container.appendChild(section);
  }

  // Restore expanded symbols after DOM is ready (next tick)
  setTimeout(async () => {
    try {
      const { restoreExpandedSymbols } = await import('./symbolPickerManagement.js');
      restoreExpandedSymbols(); // Note: this is debounced, so it's OK to call multiple times
    } catch (error) {
      // Symbol picker may not be loaded - ignore
    }
  }, 0);
}

/**
 * Tworzy sekcję błędu dla projektu
 */
function createErrorSection(projectData) {
  const projectName = projectData.projectPath.split(/[\\/]/).pop() || projectData.projectPath;
  const section = document.createElement('div');
  section.className = 'project-section error collapsed';
  section.innerHTML = `
    <div class="project-header" title="Error loading: ${projectData.projectPath}">
      <span class="expand-icon">▼</span>
      <span class="project-name">${projectName}</span>
      <span class="project-stats">Load Failed</span>
    </div>
    <div class="project-tree-nodes"><div class="error-message">${projectData.error}</div></div>`;
  section.querySelector('.project-header').addEventListener('click', () => section.classList.toggle('collapsed'));
  return section;
}

/**
 * Renderuje drzewo węzłów (rekurencyjnie)
 */
export function renderNodeTree(treeNodes, parentElement, depth, projectPath) {
  if (!parentElement) return;

  treeNodes.forEach(node => {
    const nodeElement = document.createElement('div');
    nodeElement.className = `tree-node ${node.nodeType}`;
    nodeElement.dataset.path = node.path;
    nodeElement.dataset.project = projectPath;

    const highlightedName = searchQuery && node.name.toLowerCase().includes(searchQuery)
      ? node.name.replace(new RegExp(`(${searchQuery})`, 'gi'), '<mark>$1</mark>')
      : node.name;

    const icon = node.nodeType === 'directory' ? '📁' : '📄';
    nodeElement.innerHTML = `<span class="tree-icon">${icon}</span><span class="tree-name">${highlightedName}</span>`;

    if (node.nodeType === 'file') {
      // Click handlers for file nodes
      nodeElement.addEventListener('click', async (event) => {
        event.stopPropagation();

        if (event.ctrlKey || event.metaKey) {
          // Ctrl+Click = toggle symbol expansion
          const { toggleFileSymbols } = await import('./symbolPickerManagement.js');
          toggleFileSymbols(nodeElement, node.path, projectPath);
        } else {
          // Normal click = select file for context
          handleNodeClick(node, nodeElement, projectPath);
        }
      });

      // Double-click adds full file to context
      nodeElement.addEventListener('dblclick', (event) => {
        event.stopPropagation();
        handleFileDoubleClick(node, projectPath);
      });

      if (selectedNodes.get(projectPath)?.has(node.path)) {
        nodeElement.classList.add('selected');
      }
    } else if (node.nodeType === 'directory') {
      const uniqueId = `${projectPath}::${node.path}`;
      nodeElement.addEventListener('click', (event) => {
        event.stopPropagation();

        if (event.ctrlKey || event.metaKey) {
          if (collapsedNodes.has(uniqueId)) {
            collapsedNodes.delete(uniqueId);
          } else {
            collapsedNodes.add(uniqueId);
          }
          toggleFolderCollapse(nodeElement);
        } else {
          selectAllInFolder(node, projectPath);
        }
      });

      // Dodaj obsługę podwójnego kliknięcia dla katalogów
      nodeElement.addEventListener('dblclick', (event) => {
        event.stopPropagation();
        handleDirectoryDoubleClick(node, projectPath);
      });

      if (node.children && node.children.length > 0) {
        const filesInFolder = collectFilesFromFolder(node);
        const projectSelection = selectedNodes.get(projectPath);
        if (projectSelection) {
          const selectedCount = filesInFolder.filter(path => projectSelection.has(path)).length;
          if (selectedCount > 0) {
            nodeElement.classList.add('partially-selected');
            if (selectedCount === filesInFolder.length) {
              nodeElement.classList.add('fully-selected');
            }
          }
        }
      }
    }

    parentElement.appendChild(nodeElement);

    if (node.children && node.children.length > 0) {
      const childrenContainer = document.createElement('div');
      childrenContainer.className = 'tree-children';
      const uniqueId = `${projectPath}::${node.path}`;

      if (collapsedNodes.has(uniqueId)) {
        childrenContainer.style.display = 'none';
        nodeElement.classList.add('collapsed');
      }

      parentElement.appendChild(childrenContainer);
      renderNodeTree(node.children, childrenContainer, depth + 1, projectPath);
    }
  });
}

/**
 * Obsługuje kliknięcie na węzeł (plik)
 */
export function handleNodeClick(node, nodeElement, projectPath) {
  if (!selectedNodes.has(projectPath)) {
    selectedNodes.set(projectPath, new Set());
  }
  const projectSelection = selectedNodes.get(projectPath);
  if (projectSelection.has(node.path)) {
    projectSelection.delete(node.path);
  } else {
    projectSelection.add(node.path);
  }
  nodeElement.classList.toggle('selected');
  updateProjectStats(projectPath);
  updateSelectionInfo();
}

/**
 * Obsługuje podwójne kliknięcie na plik
 */
export function handleFileDoubleClick(node, projectPath) {
  const extension = node.name.split('.').pop().toLowerCase();

  if (BINARY_EXTENSIONS.has(extension)) {
    showStatusMessage(`Preparing ${node.name} for upload...`, 'info');
    const fullPath = `${projectPath.replace(/\\/g, '/')}/${node.path}`;
    fileTreeLogger.log(`Sending 'get_binary_file_for_upload' for path: ${fullPath}`);
    chrome.runtime.sendMessage({
      action: 'get_binary_file_for_upload',
      payload: { filepath: fullPath }
    });
  } else {
    showStatusMessage(`Attaching ${node.name}...`, 'info');
    setLastAction('attach');
    chrome.runtime.sendMessage({
      action: 'get_files_multi',
      payload: {
        projects: [{
          rootPath: projectPath,
          relativePaths: [node.path]
        }]
      }
    });
  }
}

/**
 * Obsługuje podwójne kliknięcie na katalog
 * Zbiera wszystkie pliki z katalogu i załącza je jako jeden plik tekstowy
 */
export function handleDirectoryDoubleClick(node, projectPath) {
  // Zbierz wszystkie pliki z katalogu rekurencyjnie
  const filesInDirectory = collectFilesFromFolder(node);

  if (filesInDirectory.length === 0) {
    showStatusMessage(`Folder ${node.name} is empty`, 'info');
    return;
  }

  // Filtruj tylko pliki tekstowe (pomijamy binarne)
  const textFiles = filesInDirectory.filter(filePath => {
    const extension = filePath.split('.').pop().toLowerCase();
    return !BINARY_EXTENSIONS.has(extension);
  });

  if (textFiles.length === 0) {
    showStatusMessage(`No text files in folder ${node.name}`, 'info');
    return;
  }

  showStatusMessage(`Attaching ${textFiles.length} files from ${node.name}...`, 'info');
  setLastAction('attach');

  chrome.runtime.sendMessage({
    action: 'get_files_multi',
    payload: {
      projects: [{
        rootPath: projectPath,
        relativePaths: textFiles
      }]
    }
  });
}

/**
 * Zaznacza wszystkie pliki w projekcie
 */
export function handleSelectAllInProject(projectPath, tree) {
  const allFiles = (nodes, paths = []) => {
    for (const node of nodes) {
      if (node.nodeType === 'file') {
        paths.push(node.path);
      } else if (node.children) {
        allFiles(node.children, paths);
      }
    }
    return paths;
  };
  selectedNodes.set(projectPath, new Set(allFiles(tree)));
  const dataToRender = searchQuery ? filterFileTree(fileTreeData) : fileTreeData;
  renderMergedFileTree(dataToRender);
  updateProjectStats(projectPath);
  updateSelectionInfo();
}

/**
 * Czyści selekcję w projekcie
 */
export function handleClearAllInProject(projectPath) {
  if (selectedNodes.has(projectPath)) {
    selectedNodes.get(projectPath).clear();
  }
  const dataToRender = searchQuery ? filterFileTree(fileTreeData) : fileTreeData;
  renderMergedFileTree(dataToRender);
  updateProjectStats(projectPath);
  updateSelectionInfo();
}

/**
 * Aktualizuje statystyki projektu
 */
export function updateProjectStats(projectPath) {
  const statsEl = document.getElementById(`stats-${btoa(projectPath)}`);
  if (!statsEl) return;

  const projectData = fileTreeData.find(p => p.projectPath === projectPath);
  if (!projectData) return;

  const selectedCount = selectedNodes.get(projectPath)?.size || 0;
  statsEl.textContent = selectedCount > 0
    ? `${projectData.fileCount} files, ${selectedCount} sel.`
    : `${projectData.fileCount} files`;
}

/**
 * Zbiera wszystkie pliki z folderu (rekurencyjnie)
 */
export function collectFilesFromFolder(folderNode) {
  const filePaths = [];

  function traverse(node) {
    if (node.nodeType === 'file') {
      filePaths.push(node.path);
    } else if (node.nodeType === 'directory' && node.children) {
      node.children.forEach(child => traverse(child));
    }
  }

  if (folderNode.children) {
    folderNode.children.forEach(child => traverse(child));
  }

  return filePaths;
}

/**
 * Zaznacza/odznacza wszystkie pliki w folderze
 */
export function selectAllInFolder(folderNode, projectPath) {
  if (!selectedNodes.has(projectPath)) {
    selectedNodes.set(projectPath, new Set());
  }

  const projectSelection = selectedNodes.get(projectPath);
  const filesInFolder = collectFilesFromFolder(folderNode);

  const allSelected = filesInFolder.every(path => projectSelection.has(path));

  if (allSelected) {
    filesInFolder.forEach(path => projectSelection.delete(path));
  } else {
    filesInFolder.forEach(path => projectSelection.add(path));
  }

  const dataToRender = searchQuery ? filterFileTree(fileTreeData) : fileTreeData;
  renderMergedFileTree(dataToRender);
  updateProjectStats(projectPath);
  updateSelectionInfo();
}

/**
 * Przełącza collapse/expand dla folderu
 */
export function toggleFolderCollapse(nodeElement) {
  const childrenContainer = nodeElement.nextElementSibling;

  if (!childrenContainer || !childrenContainer.classList.contains('tree-children')) {
    return;
  }

  nodeElement.classList.toggle('collapsed');
  const isCollapsed = childrenContainer.style.display === 'none';

  if (isCollapsed) {
    childrenContainer.style.display = 'block';
    const icon = nodeElement.querySelector('.tree-icon');
    if (icon) icon.textContent = '📂';
  } else {
    childrenContainer.style.display = 'none';
    const icon = nodeElement.querySelector('.tree-icon');
    if (icon) icon.textContent = '📁';
  }
}

// ============================================================================
// Search & Filter
// ============================================================================

/**
 * Obsługuje wyszukiwanie
 */
export function handleSearch(event) {
  setSearchQuery(event.target.value.toLowerCase().trim());
  clearTimeout(searchTimeout);
  const timeout = setTimeout(() => {
    if (fileTreeData && fileTreeData.length > 0) {
      const filteredData = searchQuery ? filterFileTree(fileTreeData) : fileTreeData;
      renderMergedFileTree(filteredData);
    }
  }, 300);
  setSearchTimeout(timeout);
}

/**
 * Filtruje drzewo plików
 */
export function filterFileTree(projects) {
  return projects.reduce((acc, project) => {
    if (project.error) {
      acc.push(project);
      return acc;
    }
    const filterNodes = (nodes) => {
      return nodes.reduce((subAcc, node) => {
        const isMatch = node.name.toLowerCase().includes(searchQuery);
        if (node.nodeType === 'directory') {
          const filteredChildren = filterNodes(node.children || []);
          if (filteredChildren.length > 0 || isMatch) {
            subAcc.push({ ...node, children: filteredChildren });
          }
        } else if (isMatch) {
          subAcc.push(node);
        }
        return subAcc;
      }, []);
    };
    const filteredTree = filterNodes(project.tree);
    if (filteredTree.length > 0) {
      acc.push({ ...project, tree: filteredTree });
    }
    return acc;
  }, []);
}

// ============================================================================
// Selection Management
// ============================================================================

/**
 * Czyści selekcję
 */
export function handleClearSelection() {
  const projectsToUpdate = [...selectedNodes.keys()];
  selectedNodes.clear();
  document.querySelectorAll('.tree-node.selected').forEach(el => el.classList.remove('selected'));
  projectsToUpdate.forEach(updateProjectStats);
  updateSelectionInfo();
}

/**
 * Aktualizuje informacje o selekcji
 */
export function updateSelectionInfo() {
  const infoEl = document.getElementById('selectionInfo');
  const copyBtn = document.getElementById('copyBtn');
  const generateBtn = document.getElementById('generateBtn');
  const generateSimpleBtn = document.getElementById('generateSimpleBtn');

  let totalFiles = 0;
  let projectCount = 0;
  selectedNodes.forEach(filesSet => {
    if (filesSet.size > 0) {
      totalFiles += filesSet.size;
      projectCount++;
    }
  });

  if (totalFiles > 0) {
    infoEl.style.display = 'flex';
    document.getElementById('selectionCount').textContent = `${totalFiles} file${totalFiles > 1 ? 's' : ''} from ${projectCount} project${projectCount > 1 ? 's' : ''}`;

    const estimatedSize = 5 * totalFiles;
    document.getElementById('selectionSize').textContent = estimatedSize < 1024
      ? `~${estimatedSize} KB`
      : `~${(estimatedSize / 1024).toFixed(1)} MB`;

    copyBtn.disabled = false;
  } else {
    infoEl.style.display = 'none';
    copyBtn.disabled = true;
  }

  const hasSelectedFiles = totalFiles > 0;

  // Przycisk "Generate" jest teraz zawsze aktywny.
  // Walidacja jest przeprowadzana w momencie kliknięcia.
  if (generateBtn) {
    generateBtn.disabled = false;
  }

  // Przycisk "Simple" jest włączony tylko, gdy wybrano pliki.
  if (generateSimpleBtn) {
    generateSimpleBtn.disabled = !hasSelectedFiles;
  }

  // Przycisk "Map" jest włączony tylko, gdy wybrano pliki.
  const generateMapBtn = document.getElementById('generateMapBtn');
  if (generateMapBtn) {
    generateMapBtn.disabled = !hasSelectedFiles;
  }
}

/**
 * Konstruuje payload dla wielu projektów
 */
export function constructMultiProjectPayload() {
  const projectsPayload = [];
  for (const [projectPath, filesSet] of selectedNodes.entries()) {
    // CRITICAL FIX: Pomiń wirtualny projekt plików, ponieważ są one wysyłane osobno
    // w polu 'virtualFiles' i nie istnieją fizycznie na dysku (unikamy OS Error 3).
    if (projectPath === VIRTUAL_FILES_PROJECT_PATH) continue;

    if (filesSet.size > 0) {
      projectsPayload.push({ rootPath: projectPath, relativePaths: Array.from(filesSet) });
    }
  }
  return projectsPayload;
}

/**
 * Enhanced version that includes selected symbols for export
 */
export async function constructMultiProjectPayloadWithSymbols() {
  const projectsPayload = [];

  // Get selected symbols (if symbol picker is loaded)
  let selectedSymbols = new Map();
  try {
    const { getSelectedSymbols } = await import('./symbolPickerManagement.js');
    selectedSymbols = getSelectedSymbols();
  } catch (e) {
    // Symbol picker not loaded - ignore
  }

  for (const [projectPath, filesSet] of selectedNodes.entries()) {
    // CRITICAL FIX: Pomiń wirtualny projekt plików, ponieważ są one wysyłane osobno
    // w polu 'virtualFiles' i nie istnieją fizycznie na dysku (unikamy OS Error 3).
    if (projectPath === VIRTUAL_FILES_PROJECT_PATH) continue;

    if (filesSet.size > 0 || selectedSymbols.size > 0) {
      const payload = {
        rootPath: projectPath,
        relativePaths: Array.from(filesSet),
        symbols: {} // Map: filePath -> [symbolNames]
      };

      // Add selected symbols for this project
      for (const [fileKey, symbolSet] of selectedSymbols.entries()) {
        const [symbolProjectPath, filePath] = fileKey.split('::');
        if (symbolProjectPath === projectPath && symbolSet.size > 0) {
          payload.symbols[filePath] = Array.from(symbolSet);
        }
      }

      projectsPayload.push(payload);
    }
  }
  return projectsPayload;
}

/**
 * Tworzy mapowanie projektów
 */
export function getProjectMapping() {
  const mapping = {};

  for (const projectPath of selectedProjects) {
    const projectName = projectPath.split(/[\\/]/).pop() || projectPath;
    const sanitizedName = projectName.replace(/[^a-zA-Z0-9_]/g, '_').toLowerCase();
    const key = `@gluon:${sanitizedName}`;
    mapping[key] = projectPath;
  }

  return mapping;
}

// ============================================================================
// File Tree Utilities
// ============================================================================

/**
 * Aktualizuje licznik plików
 */
export function updateFileCount() {
  const totalFiles = fileTreeData.reduce((sum, p) => sum + (p.fileCount || 0), 0);
  document.getElementById('fileCount').textContent = `${totalFiles} file${totalFiles !== 1 ? 's' : ''}`;
}

/**
 * Pokazuje empty state
 */
export function showEmptyState(message) {
  document.getElementById('fileTree').innerHTML = `<div class="empty-state"><div class="empty-icon">📁</div><div class="empty-text">${message}</div></div>`;
}

/**
 * Czyści drzewo plików
 */
export function clearFileTree() {
  showEmptyState('Select a project');
  setFileTreeData([]);
  updateFileCount();
}

/**
 * Znajduje i zaznacza plik w drzewie
 */
export function findAndSelectInTree(nodes, targetPath, selection) {
  for (const node of nodes) {
    if (node.path === targetPath && node.nodeType === 'file') {
      selection.add(node.path);
      return true;
    }
    if (node.children && node.children.length > 0) {
      if (findAndSelectInTree(node.children, targetPath, selection)) {
        return true;
      }
    }
  }
  return false;
}

/**
 * Znajduje plik w drzewie (bez zaznaczania)
 */
export function findFileInTree(nodes, targetPath) {
  for (const node of nodes) {
    if (node.path === targetPath && node.nodeType === 'file') {
      return true;
    }
    if (node.children && node.children.length > 0) {
      if (findFileInTree(node.children, targetPath)) {
        return true;
      }
    }
  }
  return false;
}

/**
 * Znajduje plik po nazwie (fallback)
 */
export function findAndSelectByFileName(nodes, fileName, selection) {
  for (const node of nodes) {
    if (node.nodeType === 'file' && node.name === fileName) {
      selection.add(node.path);
      return true;
    }
    if (node.children && node.children.length > 0) {
      if (findAndSelectByFileName(node.children, fileName, selection)) {
        return true;
      }
    }
  }
  return false;
}

// ============================================================================
// File Tree Polling
// ============================================================================

/**
 * Rozpoczyna polling drzewa plików
 */
export function startFileTreePolling() {
  stopFileTreePolling();
  const paths = Array.from(selectedProjects);
  if (paths.length > 0) {
    fileTreeLogger.log(`Starting file tree polling every ${POLLING_INTERVAL_MS}ms for:`, paths);
    const interval = setInterval(() => {
      chrome.runtime.sendMessage({
        action: 'get_file_trees',
        payload: { paths }
      });

      if (!window.pollCounter) window.pollCounter = 0;
      window.pollCounter++;
      if (window.pollCounter % 5 === 0) {
        // Co 5 iteracji (co 5 sekund) odśwież też historię kontekstu
        chrome.runtime.sendMessage({
          action: 'get_context_files_history',
          payload: { selectedProjects: Array.from(selectedProjects) }
        });
      }
    }, POLLING_INTERVAL_MS);
    setFileTreePollingInterval(interval);
  }
}

/**
 * Zatrzymuje polling drzewa plików
 */
export function stopFileTreePolling() {
  if (fileTreePollingInterval) {
    fileTreeLogger.log('Stopping file tree polling.');
    clearInterval(fileTreePollingInterval);
    setFileTreePollingInterval(null);
    window.pollCounter = 0;
  }
}

/**
 * Konfiguruje resizer dla drzewa plików
 */
export function setupFileTreeResizer() {
  const container = document.getElementById('fileTreeContainer');
  const resizer = document.getElementById('fileTreeResizer');

  if (!container || !resizer) return;

  let isResizing = false;
  let startY = 0;
  let startHeight = 0;

  chrome.storage.local.get({ fileTreeHeight: 400 }, (data) => {
    container.style.height = `${data.fileTreeHeight}px`;
  });

  resizer.addEventListener('mousedown', (e) => {
    isResizing = true;
    startY = e.clientY;
    startHeight = container.offsetHeight;

    resizer.classList.add('resizing');
    document.body.classList.add('resizing');

    e.preventDefault();
  });

  document.addEventListener('mousemove', (e) => {
    if (!isResizing) return;

    const delta = e.clientY - startY;
    const newHeight = Math.max(200, Math.min(800, startHeight + delta));

    container.style.height = `${newHeight}px`;

    e.preventDefault();
  });

  document.addEventListener('mouseup', () => {
    if (!isResizing) return;

    isResizing = false;
    resizer.classList.remove('resizing');
    document.body.classList.remove('resizing');

    const currentHeight = container.offsetHeight;
    chrome.storage.local.set({ fileTreeHeight: currentHeight });

    fileTreeLogger.log('File tree height saved:', currentHeight);
  });
}