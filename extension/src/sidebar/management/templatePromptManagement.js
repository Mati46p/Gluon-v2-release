// ============================================================================
// Template & Prompt Management Module
// Zarządza szablonami, promptami i środowiskami
// ============================================================================

import {
  environments, selectedEnvironmentId, enabledPromptIds, templates, activeTemplates,
  selectedProjects,
  setEnvironments, setSelectedEnvironmentId, setEnabledPromptIds, setTemplates,
  setActiveTemplates,
  showStatusMessage
} from './stateManagement.js';

import { getProjectMapping } from './fileTreeManagement.js';
import { templateLogger } from '../../common/logger.js';
import { generatePrompt } from '../utils/prompt-generator.js';
import {
  savePromptToHistory,
  PromptHistoryNavigator
} from './promptHistoryManagement.js';

// ============================================================================
// Prompt History Navigator
// ============================================================================

let promptNavigator = null;

/**
 * Obsługuje nawigację strzałkami w historii promptów
 */
export function handlePromptHistoryKeydown(event) {
  if (promptNavigator) {
    promptNavigator.handleKeyDown(event);
  }
}

// ============================================================================
// Environment Management
// ============================================================================

/**
 * Konfiguruje event listenery dla środowisk
 */
export function setupEnvironmentListeners() {
  document.getElementById('environmentSelect').addEventListener('change', handleEnvironmentChange);

  document.addEventListener('visibilitychange', () => {
    if (!document.hidden) {
      templateLogger.log('Tab became visible - refreshing environments');
      chrome.runtime.sendMessage({ action: 'get_environments' });
    }
  });
}

/**
 * Ładuje środowiska
 */
export function loadEnvironments() {
  showStatusMessage('Loading environments...', 'info');
  chrome.runtime.sendMessage({ action: 'get_environments' });
}

/**
 * Wypełnia listę środowisk
 */
export function populateEnvironments() {
  const select = document.getElementById('environmentSelect');
  select.innerHTML = '<option value="">Select an environment...</option>';

  if (!environments || environments.length === 0) {
    select.innerHTML = '<option value="">No environments found. Add one in the app.</option>';
    return;
  }

  environments.forEach(envData => {
    const option = document.createElement('option');
    option.value = envData.id;
    option.textContent = `${envData.icon} ${envData.name}`;
    if (envData.isDefault) {
      option.textContent;
    }
    select.appendChild(option);
  });

  if (selectedEnvironmentId && environments.some(e => e.id == selectedEnvironmentId)) {
    select.value = selectedEnvironmentId;
  } else {
    const defaultEnv = environments.find(e => e.isDefault);
    if (defaultEnv) {
      select.value = defaultEnv.id;
      setSelectedEnvironmentId(defaultEnv.id);
    }
  }

  renderPrompts();
}

/**
 * Resetuje środowisko do domyślnego
 */
export async function resetToDefaultEnvironment() {
  if (!environments || environments.length === 0) {
    return;
  }

  const defaultEnv = environments.find(e => e.isDefault);
  if (defaultEnv && selectedEnvironmentId !== defaultEnv.id) {
    setSelectedEnvironmentId(defaultEnv.id);
    await chrome.storage.local.set({ selectedEnvironmentId: defaultEnv.id });

    const select = document.getElementById('environmentSelect');
    if (select) {
      select.value = defaultEnv.id;
    }

    // Reset enabled prompts to default
    enabledPromptIds.clear();
    defaultEnv.prompts.forEach(p => {
      if (p.enabled_by_default) {
        enabledPromptIds.add(p.id);
      }
    });
    await chrome.storage.local.set({ enabledPromptIds: Array.from(enabledPromptIds) });

    renderPrompts();
    templateLogger.log('Environment reset to default:', defaultEnv.name);
  }
}

/**
 * Ustawia środowisko na podstawie danych z projektu
 */
export async function setEnvironmentForProject(environmentData) {
  templateLogger.log('setEnvironmentForProject called with data:', environmentData);

  // Backend uses #[serde(flatten)] so environment fields are at root level
  // Check if we have a valid environment object (either nested or flattened)
  const env = environmentData?.environment || (environmentData?.id ? environmentData : null);

  if (!env || !env.id) {
    templateLogger.log('No valid environment data provided, resetting to default');
    await resetToDefaultEnvironment();
    return;
  }

  // Check if this environment exists in our loaded environments list
  const envExists = environments.some(e => e.id === env.id);
  if (!envExists) {
    templateLogger.warn('Environment', env.name, 'not found in loaded environments list');
    await resetToDefaultEnvironment();
    return;
  }

  const prompts = environmentData.prompts || [];

  templateLogger.log('Setting environment to:', env.name, 'with', prompts.length, 'prompts');

  // Update environment if different
  if (selectedEnvironmentId !== env.id) {
    setSelectedEnvironmentId(env.id);
    await chrome.storage.local.set({ selectedEnvironmentId: env.id });

    const select = document.getElementById('environmentSelect');
    if (select) {
      select.value = env.id;
    }

    // Set prompts based on project's environment
    enabledPromptIds.clear();
    prompts.forEach(p => {
      if (p.enabled_by_default) {
        enabledPromptIds.add(p.id);
      }
    });
    await chrome.storage.local.set({ enabledPromptIds: Array.from(enabledPromptIds) });

    renderPrompts();
    templateLogger.log('Environment set to project assignment:', env.name);
  } else {
    templateLogger.log('Environment already set to:', env.name);
  }
}

/**
 * Obsługuje zmianę środowiska
 */
export async function handleEnvironmentChange(event) {
  const newId = event.target.value ? parseInt(event.target.value, 10) : null;
  if (selectedEnvironmentId === newId) return;

  setSelectedEnvironmentId(newId);
  await chrome.storage.local.set({ selectedEnvironmentId: newId });

  enabledPromptIds.clear();
  const envData = environments.find(e => e.id === selectedEnvironmentId);
  if (envData) {
    envData.prompts.forEach(p => {
      if (p.enabled_by_default) {
        enabledPromptIds.add(p.id);
      }
    });
  }
  await chrome.storage.local.set({ enabledPromptIds: Array.from(enabledPromptIds) });

  renderPrompts();
}

// ============================================================================
// Prompt Management
// ============================================================================

/**
 * Renderuje listę promptów
 */
export function renderPrompts() {
  const promptsSection = document.getElementById('promptsSection');
  const promptsList = document.getElementById('promptsList');
  promptsList.innerHTML = '';

  if (!selectedEnvironmentId) {
    promptsSection.style.display = 'none';
    updatePromptsCount();
    return;
  }

  const envData = environments.find(e => e.id === selectedEnvironmentId);
  if (!envData || !envData.prompts || envData.prompts.length === 0) {
    promptsSection.style.display = 'none';
    updatePromptsCount();
    return;
  }

  promptsSection.style.display = 'block';

  envData.prompts.forEach(prompt => {
    const isChecked = enabledPromptIds.has(prompt.id);
    const item = document.createElement('label');
    item.className = 'prompt-item';
    item.innerHTML = `
      <input type="checkbox" data-prompt-id="${prompt.id}" ${isChecked ? 'checked' : ''}>
      <div class="prompt-details">
        <span class="prompt-name">${prompt.name}</span>
        <span class="prompt-category-badge ${prompt.category}">${prompt.category}</span>
        <span class="prompt-preview" title="${prompt.content || ''}">${prompt.content || 'No preview available.'}</span>
      </div>
    `;
    const checkbox = item.querySelector('input');
    checkbox.addEventListener('change', (e) => handlePromptToggle(prompt.id, e.target.checked));

    promptsList.appendChild(item);
  });
  updatePromptsCount();
}

/**
 * Obsługuje toggle prompta
 */
export async function handlePromptToggle(promptId, isEnabled) {
  if (isEnabled) {
    enabledPromptIds.add(promptId);
  } else {
    enabledPromptIds.delete(promptId);
  }
  await chrome.storage.local.set({ enabledPromptIds: Array.from(enabledPromptIds) });
  updatePromptsCount();
}

/**
 * Aktualizuje licznik promptów
 */
function updatePromptsCount() {
  const count = enabledPromptIds.size;
  const countEl = document.getElementById('promptsCount');
  countEl.textContent = `${count} selected`;
}

// ============================================================================
// Template Management
// ============================================================================

/**
 * Ładuje szablony
 */
export async function loadTemplates() {
  templateLogger.log('Loading templates...');
  const data = await chrome.storage.local.get({ gluon_templates: null });

  let loadedTemplates = data.gluon_templates || {};
  let templatesUpdated = false;

  const defaultTemplates = {
    auto_select: {
      id: 'default',
      name: {
        en: 'Default Auto-Select',
        pl: 'Domyślny Auto-Select'
      },
      locked: true,
      systemPrompt: {
        en: `// Role / Behavior Definition
  Analyze the user's query and identify the set of files necessary to solve the problem. Select only those files that will be directly modified or are critical to understanding the task's context.`,
        pl: `// Rola / Definicja Zachowania
  Przeanalizuj zapytanie użytkownika i zidentyfikuj zestaw plików niezbędnych do rozwiązania problemu. Wybierz tylko te pliki, które będą bezpośrednio modyfikowane lub są konieczne do zrozumienia kontekstu zadania.`
      }
    },
    context_handoff: {
      id: 'default',
      name: {
        en: 'Default Context Handoff',
        pl: 'Domyślne Przekazanie Kontekstu'
      },
      locked: true,
      systemPrompt: {
        en: `// Role / Behavior Definition
Create a complete context handoff package to continue the conversation in a new thread. The package must include a full history of the work, all key architectural decisions, and the detailed status of the task, so that a new model can take over without losing context.

// Instructions for field: summary
DETAILED CHRONOLOGY: Describe the entire thread's progress step-by-step in chronological order. For each stage, specify: what was done, what decisions were made, and why a particular approach was chosen.

// Instructions for field: solved_problems
List each solved problem, detailing the description, the implemented solution, the files modified, and the rationale for the approach.

// Instructions for field: current_problem
CURRENT WORK STATUS: Describe in detail what is currently being worked on, what exactly is being done at this moment, what the progress is, and what specific challenges have arisen.

// Instructions for field: key_insights
CRITICAL CONTEXTUAL INFORMATION: Record all significant technical discoveries, architectural decisions, known system limitations, project specifics, issues to avoid, adopted coding conventions, and dependencies between components.`,
        pl: `// Rola / Definicja Zachowania
Stwórz kompletny pakiet do przekazania kontekstu, aby kontynuować rozmowę w nowym wątku. Pakiet musi zawierać pełną historię pracy, wszystkie kluczowe decyzje architektoniczne oraz szczegółowy status zadania, tak aby nowy model mógł przejąć pracę bez utraty kontekstu.

// Instrukcje dla pola: summary
SZCZEGÓŁOWA CHRONOLOGIA: Opisz krok po kroku cały postęp wątku w porządku chronologicznym. Dla każdego etapu określ: co zostało zrobione, jakie decyzje podjęto i dlaczego wybrano dane podejście.

// Instrukcje dla pola: solved_problems
Wymień każdy rozwiązany problem, podając jego opis, zaimplementowane rozwiązanie, zmodyfikowane pliki oraz uzasadnienie podejścia.

// Instrukcje dla pola: current_problem
AKTUALNY STAN PRAC: Opisz szczegółowo, nad czym obecnie pracujesz, co dokładnie jest robione w tej chwili, jaki jest postęp i jakie konkretne wyzwania się pojawiły.

// Instrukcje dla pola: key_insights
KRYTYCZNE INFORMACJE KONTEKSTOWE: Zapisz wszystkie istotne odkrycia techniczne, decyzje architektoniczne, znane ograniczenia systemu, specyfikę projektu, problemy, których należy unikać, przyjęte konwencje kodowania i zależności między komponentami.`
      }
    },
    prompt_handoff: {
      id: 'default',
      name: {
        en: 'Default Prompt Handoff',
        pl: 'Domyślne Przekazanie Promptu'
      },
      locked: true,
      systemPrompt: {
        en: `// Role / Behavior Definition
Your role is an experienced software architect. Your task is to generate a complete technical specification of tasks in the form of a prompt that will allow the coding model to implement the solution without additional questions. Fill all sections of the JSON response format based on user data. Obtain as much information as possible about the task from the user and provide the JSON prompt only after the plan has been approved by the user.

// Instructions for field: task_description
Describe exactly what needs to be implemented and why, covering both the business and technical goals.

// Instructions for field: implementation_steps
Create an atomic, step-by-step implementation plan. Each step should be a clear, actionable instruction referring to a specific file or component.

// Instructions for field: technologies
List the key technologies, libraries, frameworks, and tools that must be used for the implementation, consistent with the project's existing stack.

// Instructions for field: architecture
Provide a detailed description of how the new components will integrate with the existing code, the step-by-step data flow, calls between functions, business logic, and error handling.

// Instructions for field: code_context
For any code elements that will be used but are NOT attached as files, provide their path, name, signature, and a description of their functionality. The goal is to prevent the programmer AI from needing to look for information in other files.`,
      pl: `// Rola / Definicja Zachowania
Twoja rola to doświadczony architekt oprogramowania. Twoim zadaniem jest wygenerowanie kompletnej specyfikacji technicznej tasków w formie promptu, która pozwoli modelowi kodującemu zaimplementować rozwiązanie bez dodatkowych pytań. Wypełnij wszystkie sekcje formatu odpowiedzi JSON na podstawie danych użytkownika. Uzyskaj maksymalnie dużo inforamcji na temat zadania, od użytkownia i podaj prompt JSON dopiero po zaakceptowaniu planu przez użytkownika.

// Instrukcje dla pola: task_description
Opisz dokładnie, co należy zaimplementować i dlaczego, uwzględniając zarówno cele biznesowe, jak i techniczne.

// Instrukcje dla pola: implementation_steps
Stwórz atomowy, krokowy plan implementacji. Każdy krok powinien być jasną, wykonalną instrukcją odnoszącą się do konkretnego pliku lub komponentu.

// Instrukcje dla pola: technologies
Wymień kluczowe technologie, biblioteki, frameworki i narzędzia, które muszą zostać użyte do implementacji, zgodnie z istniejącym stosem technologicznym projektu.

// Instrukcje dla pola: architecture
Dostarcz szczegółowy opis, jak nowe komponenty zintegrują się z istniejącym kodem, jak będzie wyglądał przepływ danych krok po kroku, wywołania między funkcjami, logika biznesowa i obsługa błędów.

// Instrukcje dla pola: code_context
Dla wszelkich elementów kodu, które będą używane, ale NIE SĄ załączone jako pliki, podaj ich ścieżkę, nazwę, sygnaturę i opis funkcjonalności. Celem jest zapobieżenie sytuacji, w której AI-programista musi szukać informacji w innych plikach.`
      }
    },
    interactive_mode: {
      id: 'default',
      name: {
        en: 'Interactive Context Session',
        pl: 'Sesja Interaktywna (Context Architect)'
      },
      locked: true,
      systemPrompt: {
        en: `// Role / Behavior Definition
You are the Context Architect. You have access to the "Project Skeleton" but NOT the full code. Your goal is to navigate the project, request only the code snippets you need using the "context_ops" JSON protocol, and solve the user's task iteratively.

// Instructions
1. Analyze the skeleton to find relevant files.
2. If you need to read code, output the "@gluon:next_step" JSON with "file_symbol" requests.
3. If you need to search by concept, use "rag_search".
4. Once you have enough context, provide the solution.`,
        pl: `// Rola / Definicja Zachowania
Jesteś Architektem Kontekstu. Masz dostęp do "Szkieletu Projektu", ale NIE do pełnego kodu. Twoim celem jest nawigacja po projekcie, żądanie tylko tych fragmentów kodu, których potrzebujesz (używając protokołu JSON "context_ops") i iteracyjne rozwiązanie zadania użytkownika.

// Instrukcje
1. Przeanalizuj szkielet, aby znaleźć odpowiednie pliki.
2. Jeśli musisz przeczytać kod, wygeneruj JSON "@gluon:next_step" z żądaniami "file_symbol".
3. Jeśli musisz szukać według konceptu, użyj "rag_search".
4. Gdy masz wystarczający kontekst, podaj rozwiązanie.`
      }
    }
  };

  for (const type in defaultTemplates) {
    const customTemplates = (loadedTemplates[type] || []).filter(t => t.id !== 'default');
    const newTemplatesForType = [defaultTemplates[type], ...customTemplates];

    if (JSON.stringify(loadedTemplates[type]) !== JSON.stringify(newTemplatesForType)) {
      loadedTemplates[type] = newTemplatesForType;
      templatesUpdated = true;
    }
  }

  if (templatesUpdated) {
    templateLogger.log('Default templates initialized or updated. Saving...');
    await chrome.storage.local.set({ gluon_templates: loadedTemplates });
  }

  setTemplates(loadedTemplates);
  templateLogger.log('Templates loaded:', loadedTemplates);
}

/**
 * Pokazuje modal tworzenia szablonu
 */
export function showCreateTemplateModal(type = 'auto_select', templateToEdit = null, isReadOnly = false) {
  templateLogger.log('Opening modal with template:', JSON.parse(JSON.stringify(templateToEdit)), 'ReadOnly:', isReadOnly);
  const form = document.getElementById('createTemplateForm');
  form.reset();
  form.classList.toggle('readonly', isReadOnly);
  form.dataset.currentType = type;

  const deleteBtn = document.getElementById('deleteTemplateBtn');
  const saveBtn = form.querySelector('button[type="submit"]');
  const modalTitle = document.querySelector('#createTemplateModal .modal-title');

  if (isReadOnly) {
    if (modalTitle) modalTitle.textContent = 'View Template';
    if (saveBtn) saveBtn.style.display = 'none';
    if (deleteBtn) deleteBtn.style.display = 'none';
  } else if (templateToEdit) {
    if (modalTitle) modalTitle.textContent = 'Edit Template';
    if (saveBtn) {
      saveBtn.style.display = 'inline-block';
      saveBtn.textContent = 'Save Changes';
    }
    if (deleteBtn) {
      deleteBtn.style.display = 'inline-block';
      deleteBtn.dataset.type = type;
      deleteBtn.dataset.id = templateToEdit.id;
    }
  } else {
    if (modalTitle) modalTitle.textContent = 'Create New Template';
    if (saveBtn) {
      saveBtn.style.display = 'inline-block';
      saveBtn.textContent = 'Create Template';
    }
    if (deleteBtn) deleteBtn.style.display = 'none';
  }

  const templateTypeContainer = document.getElementById('templateTypeGroup')?.closest('.form-group');
  if (templateTypeContainer) {
    templateTypeContainer.style.display = 'none';
  }

  document.getElementById('templateId').value = templateToEdit ? templateToEdit.id : '';

  const selectedEnv = environments.find(e => e.id === selectedEnvironmentId);
  const language = selectedEnv ? selectedEnv.language : 'en';

  const templateName = (templateToEdit && typeof templateToEdit.name === 'object')
    ? templateToEdit.name[language] || templateToEdit.name['en']
    : (templateToEdit ? templateToEdit.name : '');
  document.getElementById('templateName').value = templateName;

  updateDynamicTemplateFields(type);

  const systemPromptContent = (templateToEdit && typeof templateToEdit.systemPrompt === 'object')
    ? templateToEdit.systemPrompt[language] || templateToEdit.systemPrompt['en']
    : (templateToEdit ? templateToEdit.systemPrompt : '');

  if (systemPromptContent) {
    const prompt = systemPromptContent;
    const sections = prompt.split(/\n(?=\/\/ )/g);

    sections.forEach(section => {
      const trimmedSection = section.trim();
      const roleMatch = trimmedSection.match(/^\/\/ (?:Role \/ Behavior Definition|Rola \/ Definicja Zachowania)\n([\s\S]*)/);
      if (roleMatch) {
        document.getElementById('systemRolePrompt').value = roleMatch[1].trim();
        return;
      }
      const fieldMatch = trimmedSection.match(/^\/\/ (?:Instructions for field|Instrukcje dla pola): (\w+)\n([\s\S]*)/);
      if (fieldMatch) {
        const fieldName = fieldMatch[1];
        const content = fieldMatch[2].trim();
        const textarea = document.getElementById(`field_${fieldName}`);
        if (textarea) {
          textarea.value = content;
        }
      }
    });
  }

  form.querySelectorAll('input, textarea').forEach(el => {
    if (el.type !== 'radio') {
      el.readOnly = isReadOnly;
    }
  });

  document.getElementById('createTemplateModal').style.display = 'flex';
}

/**
 * Ukrywa modal tworzenia szablonu
 */
export function hideCreateTemplateModal() {
  document.getElementById('createTemplateModal').style.display = 'none';
}

/**
 * Zapisuje szablon
 */
export async function saveTemplate() {
  const form = document.getElementById('createTemplateForm');
  const type = form.dataset.currentType;
  const name = document.getElementById('templateName').value.trim();
  const id = document.getElementById('templateId').value;

  const validTypes = ['auto_select', 'context_handoff', 'prompt_handoff', 'interactive_mode'];
  if (!validTypes.includes(type)) {
    showStatusMessage(`Error: Invalid template type "${type}". Cannot save.`, 'error');
    return;
  }

  if (!name) {
    showStatusMessage('Template name is required.', 'error');
    return;
  }

  let systemPromptParts = [];
  const roleDefinition = document.getElementById('systemRolePrompt').value.trim();
  if (roleDefinition) {
    systemPromptParts.push(`// Role / Behavior Definition\n${roleDefinition}`);
  }

  const dynamicFieldsContainer = document.getElementById(`fields_${type}`);
  if (dynamicFieldsContainer) {
    dynamicFieldsContainer.querySelectorAll('textarea').forEach(textarea => {
      const instruction = textarea.value.trim();
      if (instruction) {
        const fieldName = textarea.id.replace('field_', '');
        systemPromptParts.push(`\n// Instructions for field: ${fieldName}\n${instruction}`);
      }
    });
  }

  const systemPrompt = systemPromptParts.join('\n');

  if (id) {
    const templateIndex = templates[type]?.findIndex(t => t.id === id);
    if (templateIndex > -1) {
      templates[type][templateIndex] = { ...templates[type][templateIndex], name, systemPrompt };
      showStatusMessage(`Template '${name}' updated!`, 'success');
    } else {
      showStatusMessage('Error updating: Template not found.', 'error');
      return;
    }
  } else {
    const newTemplate = { id: `custom_${Date.now()}`, name, systemPrompt, locked: false };
    if (!templates[type]) templates[type] = [];
    templates[type].push(newTemplate);
    showStatusMessage(`Template '${name}' saved!`, 'success');
  }

  await chrome.storage.local.set({ gluon_templates: templates });
  await loadTemplates();
  hideCreateTemplateModal();
}

/**
 * Usuwa szablon
 */
export async function deleteTemplate(type, id) {
  if (!templates[type]) return;

  const templateIndex = templates[type].findIndex(t => t.id === id);
  if (templateIndex > -1) {
    const deletedName = templates[type][templateIndex].name;
    templates[type].splice(templateIndex, 1);
    await chrome.storage.local.set({ gluon_templates: templates });
    showStatusMessage(`Template "${deletedName}" deleted.`, 'success');

    await loadTemplates();
    showManageTemplatesModal();
  }
}

/**
 * Pokazuje modal zarządzania szablonami
 */
export function showManageTemplatesModal() {
  const modal = document.getElementById('manageTemplatesModal');
  const tabsContainer = modal.querySelector('.tabs');
  const tabContents = modal.querySelectorAll('.tab-content');

  const selectedEnv = environments.find(e => e.id === selectedEnvironmentId);
  const language = selectedEnv ? selectedEnv.language : 'en';

  const renderListForType = (type) => {
    const container = document.getElementById(`tab-${type}`);
    container.innerHTML = '';
    const list = document.createElement('div');
    list.className = 'template-list';

    const templatesForType = templates[type] || [];
    if (templatesForType.length > 0) {
      templatesForType.forEach(template => {
        const item = document.createElement('div');
        item.className = 'template-item';
        const isChecked = activeTemplates[type] === template.id;

        const templateName = (typeof template.name === 'object') ? template.name[language] || template.name['en'] : template.name;

        item.innerHTML = `
          <label class="template-item-label">
            <input type="radio" name="active_template_${type}" value="${template.id}" ${isChecked ? 'checked' : ''}>
            <div class="template-info">
              <div class="name">${templateName}</div>
              ${template.locked ? '<div class="tag">(Default)</div>' : ''}
            </div>
          </label>
          <div class="template-actions">
            ${template.locked
            ? `<button class="btn-sm view-btn" data-type="${type}" data-id="${template.id}">View</button>`
            : `<button class="btn-sm edit-btn" data-type="${type}" data-id="${template.id}">Edit</button>`
          }
          </div>
        `;
        item.querySelector('input[type="radio"]').addEventListener('change', async (e) => {
          activeTemplates[type] = e.target.value;
          await chrome.storage.local.set({ active_templates: activeTemplates });
          showStatusMessage(`'${templateName}' is now the active template for ${type}.`, 'success');
        });
        list.appendChild(item);
      });
    } else {
      list.innerHTML = `<div class="empty-text" style="padding: 10px;">No templates for this type.</div>`;
    }
    container.appendChild(list);
  };

  ['auto_select', 'context_handoff', 'prompt_handoff', 'interactive_mode'].forEach(renderListForType);

  tabsContainer.querySelectorAll('.tab-link').forEach(tab => {
    tab.addEventListener('click', () => {
      tabsContainer.querySelector('.active').classList.remove('active');
      tab.classList.add('active');
      tabContents.forEach(content => content.classList.remove('active'));
      document.getElementById(tab.dataset.tab).classList.add('active');
    });
  });

  modal.style.display = 'flex';
}

/**
 * Ukrywa modal zarządzania szablonami
 */
export function hideManageTemplatesModal() {
  document.getElementById('manageTemplatesModal').style.display = 'none';
}

/**
 * Obsługuje kliknięcie w modalu zarządzania szablonami
 */
export function handleManageTemplatesClick(event) {
  const target = event.target;
  const type = target.dataset.type;
  const id = target.dataset.id;

  if (!type || !id) return;

  const template = templates[type]?.find(t => t.id === id);
  if (!template) return;

  if (target.classList.contains('view-btn')) {
    hideManageTemplatesModal();
    showCreateTemplateModal(type, template, true);
  } else if (target.classList.contains('edit-btn')) {
    hideManageTemplatesModal();
    showCreateTemplateModal(type, template, false);
  }
}

/**
 * Aktualizuje dynamiczne pola szablonu
 */
export function updateDynamicTemplateFields(selectedType) {
  document.querySelectorAll('.dynamic-field-group').forEach(group => {
    group.style.display = 'none';
  });

  const groupToShow = document.getElementById(`fields_${selectedType}`);
  if (groupToShow) {
    groupToShow.style.display = 'block';
  }
}

// ============================================================================
// Prompt Input Modal
// ============================================================================

/**
 * Pokazuje modal wprowadzania promptu
 */
export async function showPromptInputModal(type) {
  const modal = document.getElementById('promptInputModal');
  const title = document.getElementById('promptModalTitle');
  const typeInput = document.getElementById('promptModalType');
  const textarea = document.getElementById('modalPromptTextarea');

  if (!modal || !title || !typeInput || !textarea) return;

  const titles = {
    auto_select: '🎯 Auto Select Task',
    context_handoff: '📋 Context Save Task',
    prompt_handoff: '✍️ Prompt Generation Task',
    interactive_mode: '🧠 Interactive Session'
  };
  title.textContent = titles[type] || 'Enter Your Task';

  typeInput.value = type;
  textarea.value = '';

  // Inicjalizuj navigator dla strzałek
  promptNavigator = new PromptHistoryNavigator(textarea, type);
  await promptNavigator.init();

  modal.style.display = 'flex';
  textarea.focus();
}

/**
 * Ukrywa modal wprowadzania promptu
 */
export function hidePromptInputModal() {
  const modal = document.getElementById('promptInputModal');
  if (modal) {
    modal.style.display = 'none';
  }
}

/**
 * Obsługuje submit modalu promptu
 */
export async function handlePromptModalSubmit(event) {
  event.preventDefault();
  const type = document.getElementById('promptModalType').value;
  const userQuery = document.getElementById('modalPromptTextarea').value.trim();

  if (!userQuery || !type) return;

  // Zapisz prompt do historii
  await savePromptToHistory(type, userQuery);

  await pasteTemplate(type, userQuery);
  hidePromptInputModal();
}

/**
 * Wkleja szablon
 */
export async function pasteTemplate(type, userQuery) {
  templateLogger.log(`Activating function of type: ${type}`);

  if (selectedProjects.size === 0) {
    showStatusMessage('Please select at least one project first.', 'error');
    return;
  }

  const selectedEnv = environments.find(e => e.id === selectedEnvironmentId);
  const language = selectedEnv ? selectedEnv.language : 'en';

  const activeTemplateId = activeTemplates[type];
  if (!activeTemplateId) {
    showStatusMessage(`No active template set for ${type}. Please select one in the Template Center.`, 'error');
    return;
  }

  const template = templates[type]?.find(t => t.id === activeTemplateId);

  if (!template) {
    showStatusMessage(`Active template with ID '${activeTemplateId}' not found.`, 'error');
    return;
  }

  if (!userQuery) {
    showStatusMessage('The task description cannot be empty.', 'error');
    return;
  }

  const localizedTemplate = { ...template };
  if (typeof template.systemPrompt === 'object') {
    localizedTemplate.systemPrompt = template.systemPrompt[language] || template.systemPrompt['en'];
  }
  const templateName = (typeof template.name === 'object') ? template.name[language] || template.name['en'] : template.name;

  const promptText = generatePrompt(type, localizedTemplate, selectedProjects, userQuery, language);

  if (promptText) {
    try {
      await navigator.clipboard.writeText(promptText);
      showStatusMessage(`Pasting '${templateName}' & copied to clipboard!`, 'success');
    } catch (err) {
      templateLogger.error('Failed to copy prompt to clipboard:', err);
      showStatusMessage(`Pasting '${templateName}' (copy to clipboard failed)`, 'error');
    }

    chrome.runtime.sendMessage({ action: 'paste_prompt', payload: promptText });
  }
}

/**
 * Konfiguruje hover dla dropdown
 */
export function setupDropdownHover(wrapperElement) {
  const dropdown = wrapperElement.querySelector('.template-dropdown');
  if (!dropdown) return;

  let hoverTimeout;

  wrapperElement.addEventListener('mouseenter', () => {
    hoverTimeout = setTimeout(() => {
      document.querySelectorAll('.template-dropdown').forEach(d => {
        if (d !== dropdown) {
          d.style.display = 'none';
        }
      });
      dropdown.style.display = 'block';
    }, 300);
  });

  wrapperElement.addEventListener('mouseleave', () => {
    clearTimeout(hoverTimeout);
    setTimeout(() => {
      if (!wrapperElement.matches(':hover')) {
        dropdown.style.display = 'none';
      }
    }, 150);
  });
}

// ============================================================================
// Helper: Generate Prompt
// ============================================================================
// Funkcja generatePrompt jest teraz importowana z '../utils/prompt-generator.js'
// i zawiera pełne wsparcie dla formatów JSON oraz krytycznych zasad