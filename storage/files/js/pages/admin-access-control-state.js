const EMPTY_CATALOGUE = { gateway_routes: [], mcp_servers: [], plugins: [], agents: [] };

const PANES = ['ac-welcome', 'ac-dept-editor', 'ac-user-matrix'];

export const DEFAULT_DEPARTMENT = 'Default';

export const DB_SOURCE_LABEL = 'database (this instance)';

let catalogue = EMPTY_CATALOGUE;
let departments = [];

const readEmbedded = (id, fallback) => {
  const el = document.getElementById(id);
  if (!el) return fallback;
  try {
    return JSON.parse(el.textContent) ?? fallback;
  } catch {
    return fallback;
  }
};

export const loadState = () => {
  catalogue = readEmbedded('ac-entity-catalogue', EMPTY_CATALOGUE);
  departments = readEmbedded('ac-departments', []);
};

export const entitySections = () => [
  { type: 'gateway_route', label: 'Gateway routes', items: catalogue.gateway_routes || [] },
  { type: 'mcp_server', label: 'MCP servers', items: catalogue.mcp_servers || [] },
  { type: 'plugin', label: 'Plugins', items: catalogue.plugins || [] },
  { type: 'agent', label: 'Agents', items: catalogue.agents || [] },
];

export const departmentNames = () => departments;

export const cloneTemplate = (id) =>
  document.getElementById(id).content.firstElementChild.cloneNode(true);

export const slot = (root, name) => root.querySelector(`[data-slot="${name}"]`);

export const setText = (root, name, value) => {
  const el = slot(root, name);
  if (el) el.textContent = value == null ? '' : String(value);
};

export const showPane = (paneId) => {
  for (const id of PANES) {
    document.getElementById(id).hidden = id !== paneId;
  }
};
