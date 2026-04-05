import React, { useState, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  useSensor,
  useSensors,
  useDroppable,
  useDraggable,
  closestCenter,
  type DragStartEvent,
  type DragMoveEvent,
  type DragEndEvent,
  type UniqueIdentifier,
} from "@dnd-kit/core";
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
  arrayMove,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { Settings, ShellWidgetConfig, RowItem, WidgetType } from "./SettingsApp";
import { AppList } from "./AppList";

interface Props {
  settings: Settings;
  onUpdate: (updates: Partial<Settings>) => void;
}

const WIDGET_OPTIONS: { value: WidgetType; label: string }[] = [
  { value: "Time", label: "Time" },
  { value: "Date", label: "Date" },
  { value: "CharacterCount", label: "Chars" },
  { value: "LineCount", label: "Lines" },
  { value: "CharacterAndLineCount", label: "Chars+Lines" },
  { value: "Battery", label: "Battery" },
  { value: "CapsLock", label: "CapsLock" },
  { value: "KeystrokeBuffer", label: "Keys" },
];

function getWidgetLabel(widgetType: string): string {
  if (widgetType.startsWith("Shell:")) {
    return widgetType.slice("Shell:".length);
  }
  const opt = WIDGET_OPTIONS.find((o) => o.value === widgetType);
  return opt?.label ?? widgetType;
}

function totalRowCount(rows: RowItem[]): number {
  return rows.reduce((sum, r) => sum + (r.type === "ModeChar" ? r.size : 1), 0);
}

function rowId(row: RowItem, index: number): string {
  return row.type === "ModeChar" ? "mode" : `widget-${index}`;
}

// Sortable row inside the indicator replica
function SortableRow({
  row,
  id,
  onRemove,
  onModeSize,
}: {
  row: RowItem;
  id: string;
  onRemove?: () => void;
  onModeSize?: (size: 1 | 2 | 3) => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id,
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.4 : 1,
  };

  if (row.type === "ModeChar") {
    const heights: Record<number, number> = { 1: 28, 2: 48, 3: 68 };
    return (
      <div
        ref={setNodeRef}
        style={style}
        className="replica-row replica-row-mode"
        {...attributes}
        {...listeners}
      >
        <button
          className="replica-row-remove"
          onClick={(e) => {
            e.stopPropagation();
            onRemove?.();
          }}
          style={{ alignSelf: "flex-end" }}
        >
          {"\u00d7"}
        </button>
        <div className="replica-mode-char" style={{ height: heights[row.size] }}>
          <span
            style={{
              fontSize: `${row.size * 14}px`,
              fontWeight: "bold",
              textTransform: "uppercase",
            }}
          >
            n
          </span>
        </div>
        <div className="mode-size-control">
          {([1, 2, 3] as const).map((s) => (
            <button
              key={s}
              className={`mode-size-btn ${row.size === s ? "active" : ""}`}
              onClick={(e) => {
                e.stopPropagation();
                onModeSize?.(s);
              }}
            >
              {s}x
            </button>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div
      ref={setNodeRef}
      style={style}
      className="replica-row replica-row-widget"
      {...attributes}
      {...listeners}
    >
      <span className="replica-row-label">{getWidgetLabel(row.widget_type)}</span>
      {onRemove && (
        <button
          className="replica-row-remove"
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
        >
          {"\u00d7"}
        </button>
      )}
    </div>
  );
}

// Draggable palette item
function PaletteItem({ id, label, disabled }: { id: string; label: string; disabled: boolean }) {
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id,
    disabled,
  });

  return (
    <div
      ref={setNodeRef}
      className={`palette-item ${disabled ? "disabled" : ""} ${isDragging ? "dragging" : ""}`}
      {...attributes}
      {...listeners}
    >
      {label}
    </div>
  );
}

// Droppable zone for the replica
function ReplicaDropZone({
  children,
  innerRef,
}: {
  children: React.ReactNode;
  innerRef?: React.RefObject<HTMLDivElement | null>;
}) {
  const { setNodeRef } = useDroppable({ id: "replica" });
  return (
    <div
      ref={(node) => {
        setNodeRef(node);
        if (innerRef) (innerRef as React.MutableRefObject<HTMLDivElement | null>).current = node;
      }}
      className="indicator-replica-inner"
    >
      {children}
    </div>
  );
}

// Drag overlay content
function DragOverlayContent({ row, label }: { row?: RowItem; label?: string }) {
  if (label) {
    return <div className="replica-row replica-row-widget drag-overlay">{label}</div>;
  }
  if (!row) return null;
  if (row.type === "ModeChar") {
    return (
      <div className="replica-row replica-row-mode drag-overlay">
        <span style={{ fontSize: "14px", fontWeight: "bold" }}>n</span>
      </div>
    );
  }
  return (
    <div className="replica-row replica-row-widget drag-overlay">
      {getWidgetLabel(row.widget_type)}
    </div>
  );
}

// Shell widget editor
interface EditingWidget {
  name: string;
  mode: "inline" | "file";
  script: string;
  script_path: string;
  interval_secs: number;
}

function newEditingWidget(): EditingWidget {
  return { name: "", mode: "inline", script: "", script_path: "", interval_secs: 10 };
}

function toEditing(config: ShellWidgetConfig): EditingWidget {
  return {
    name: config.name,
    mode: config.script_path ? "file" : "inline",
    script: config.script ?? "",
    script_path: config.script_path ?? "",
    interval_secs: config.interval_secs,
  };
}

function fromEditing(e: EditingWidget): ShellWidgetConfig {
  return {
    name: e.name.trim(),
    script: e.mode === "inline" ? e.script : undefined,
    script_path: e.mode === "file" ? e.script_path : undefined,
    interval_secs: e.interval_secs,
  };
}

export function WidgetSettings({ settings, onUpdate }: Props) {
  const [editing, setEditing] = useState<EditingWidget | null>(null);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [activeId, setActiveId] = useState<UniqueIdentifier | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);
  const replicaRef = useRef<HTMLDivElement | null>(null);
  // Store stable item rects captured at drag start (unaffected by placeholder)
  const itemRectsRef = useRef<{ top: number; bottom: number; midY: number }[]>([]);

  const rows = settings.indicator_rows;
  const used = totalRowCount(rows);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  // Snapshot item rects at drag start so placeholder insertion doesn't affect them
  const captureItemRects = () => {
    const container = replicaRef.current;
    if (!container) return;
    const children = Array.from(container.children).filter(
      (el) => !el.classList.contains("replica-row-placeholder") && !el.classList.contains("replica-empty"),
    );
    itemRectsRef.current = children.map((el) => {
      const rect = el.getBoundingClientRect();
      return { top: rect.top, bottom: rect.bottom, midY: rect.top + rect.height / 2 };
    });
  };

  // Compute insertion index from pointer Y using cached rects
  const computeInsertIndex = (pointerY: number): number => {
    const rects = itemRectsRef.current;
    for (let i = 0; i < rects.length; i++) {
      if (pointerY < rects[i].midY) return i;
    }
    return rows.length;
  };

  // All available palette items (built-in + shell widgets)
  const paletteItems = useMemo(() => {
    const items = [
      ...WIDGET_OPTIONS,
      ...settings.shell_widgets.map((w) => ({
        value: `Shell:${w.name}` as WidgetType,
        label: w.name,
      })),
    ];
    return items;
  }, [settings.shell_widgets]);

  // Which widget types are already placed
  const placedTypes = new Set(
    rows.filter((r): r is { type: "Widget"; widget_type: WidgetType } => r.type === "Widget").map((r) => r.widget_type),
  );

  const hasModeChar = rows.some((r) => r.type === "ModeChar");

  // Sortable IDs for the replica
  const sortableIds = rows.map((r, i) => rowId(r, i));

  const handleDragStart = (event: DragStartEvent) => {
    setActiveId(event.active.id);
    // Capture stable item positions before any placeholder is rendered
    captureItemRects();
  };

  const getPointerY = (event: { activatorEvent: Event; delta: { y: number } }) => {
    return (event.activatorEvent as PointerEvent).clientY + event.delta.y;
  };

  const handleDragMove = (event: DragMoveEvent) => {
    const activeIdStr = String(event.active.id);
    if (!activeIdStr.startsWith("palette-") || used >= 5) {
      setOverIndex(null);
      return;
    }
    setOverIndex(computeInsertIndex(getPointerY(event)));
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    const finalOverIndex = overIndex;
    setActiveId(null);
    setOverIndex(null);
    if (!over) return;

    const activeIdStr = String(active.id);
    const overIdStr = String(over.id);

    // Check if dragging from palette
    if (activeIdStr.startsWith("palette-")) {
      if (used >= 5) return;

      const isModeChar = activeIdStr === "palette-ModeChar";
      const newRow: RowItem = isModeChar
        ? { type: "ModeChar", size: 2 }
        : { type: "Widget", widget_type: activeIdStr.slice("palette-".length) as WidgetType };
      const newRows = [...rows];
      const insertAt = finalOverIndex ?? computeInsertIndex(getPointerY(event));
      newRows.splice(insertAt, 0, newRow);
      onUpdate({ indicator_rows: newRows });
      return;
    }

    // Reordering within replica
    const oldIndex = sortableIds.indexOf(activeIdStr);
    const newIndex = sortableIds.indexOf(overIdStr);
    if (oldIndex >= 0 && newIndex >= 0 && oldIndex !== newIndex) {
      onUpdate({ indicator_rows: arrayMove([...rows], oldIndex, newIndex) });
    }
  };

  const handleDragCancel = () => {
    setActiveId(null);
    setOverIndex(null);
  };

  const handleRemoveRow = (index: number) => {
    const newRows = rows.filter((_, i) => i !== index);
    onUpdate({ indicator_rows: newRows });
  };

  const handleModeSize = (size: 1 | 2 | 3) => {
    // Check if changing size would exceed limit
    const modeRow = rows.find((r): r is { type: "ModeChar"; size: 1 | 2 | 3 } => r.type === "ModeChar");
    if (!modeRow) return;
    const newUsed = used - modeRow.size + size;
    if (newUsed > 5) return;

    const newRows = rows.map((r) => (r.type === "ModeChar" ? { ...r, size } : r));
    onUpdate({ indicator_rows: newRows });
  };

  // Get overlay content for active drag
  const activeRow = activeId
    ? rows[sortableIds.indexOf(String(activeId))]
    : undefined;
  const activePaletteLabel = activeId && String(activeId).startsWith("palette-")
    ? paletteItems.find((p) => `palette-${p.value}` === String(activeId))?.label
    : undefined;

  const isPaletteDrag = activeId && String(activeId).startsWith("palette-");

  // Blue replica background
  const replicaBg = "rgb(56, 132, 244)";

  // Electron apps handlers
  const handleAddElectronApp = async () => {
    try {
      const bundleId = await invoke<string | null>("pick_app");
      if (bundleId && !settings.electron_apps.includes(bundleId)) {
        onUpdate({ electron_apps: [...settings.electron_apps, bundleId] });
      }
    } catch (e) {
      console.error("Failed to pick app:", e);
    }
  };

  const handleAddManualElectronApp = (bundleId: string) => {
    if (!settings.electron_apps.includes(bundleId)) {
      onUpdate({ electron_apps: [...settings.electron_apps, bundleId] });
    }
  };

  const handleRemoveElectronApp = (bundleId: string) => {
    onUpdate({ electron_apps: settings.electron_apps.filter((id) => id !== bundleId) });
  };

  // Shell widget handlers
  const handleNewWidget = () => {
    setEditing(newEditingWidget());
    setEditingIndex(null);
  };

  const handleEditWidget = (index: number) => {
    setEditing(toEditing(settings.shell_widgets[index]));
    setEditingIndex(index);
  };

  const handleDeleteWidget = (index: number) => {
    const widget = settings.shell_widgets[index];
    const shellValue = `Shell:${widget.name}`;
    const newRows = rows.filter(
      (r) => !(r.type === "Widget" && r.widget_type === shellValue),
    );
    onUpdate({
      shell_widgets: settings.shell_widgets.filter((_, i) => i !== index),
      indicator_rows: newRows,
    });
  };

  const handleSaveWidget = () => {
    if (!editing || !editing.name.trim()) return;
    const config = fromEditing(editing);
    const widgets = [...settings.shell_widgets];

    if (editingIndex !== null) {
      const oldName = settings.shell_widgets[editingIndex].name;
      let newRows = rows;
      if (oldName !== config.name) {
        newRows = rows.map((r) =>
          r.type === "Widget" && r.widget_type === `Shell:${oldName}`
            ? { ...r, widget_type: `Shell:${config.name}` as WidgetType }
            : r,
        );
      }
      widgets[editingIndex] = config;
      onUpdate({ shell_widgets: widgets, indicator_rows: newRows });
    } else {
      if (widgets.some((w) => w.name === config.name)) return;
      widgets.push(config);
      onUpdate({ shell_widgets: widgets });
    }
    setEditing(null);
    setEditingIndex(null);
  };

  const handleCancelEdit = () => {
    setEditing(null);
    setEditingIndex(null);
  };

  return (
    <div className="settings-section">
      <h2>Widgets</h2>

      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragStart={handleDragStart}
        onDragMove={handleDragMove}
        onDragEnd={handleDragEnd}
        onDragCancel={handleDragCancel}
      >
        <div className="widget-layout-editor">
          {/* Widget palette */}
          <div className="widget-palette">
            <div className="palette-label">Available widgets</div>
            <PaletteItem
              id="palette-ModeChar"
              label="Mode"
              disabled={hasModeChar || used >= 5}
            />
            {paletteItems.map((item) => (
              <PaletteItem
                key={item.value}
                id={`palette-${item.value}`}
                label={item.label}
                disabled={placedTypes.has(item.value) || used >= 5}
              />
            ))}
          </div>

          {/* Indicator replica */}
          <div className="indicator-replica" style={{ background: replicaBg }}>
            <div className="replica-capacity">{used}/5 rows</div>
            <SortableContext items={sortableIds} strategy={verticalListSortingStrategy}>
              <ReplicaDropZone innerRef={replicaRef}>
                {rows.map((row, i) => (
                  <React.Fragment key={rowId(row, i)}>
                    {isPaletteDrag && overIndex === i && (
                      <div className="replica-row replica-row-placeholder" />
                    )}
                    <SortableRow
                      id={rowId(row, i)}
                      row={row}
                      onRemove={() => handleRemoveRow(i)}
                      onModeSize={row.type === "ModeChar" ? handleModeSize : undefined}
                    />
                  </React.Fragment>
                ))}
                {isPaletteDrag && overIndex !== null && overIndex >= rows.length && (
                  <div className="replica-row replica-row-placeholder" />
                )}
                {rows.length === 0 && !isPaletteDrag && (
                  <div className="replica-empty">Drop widgets here</div>
                )}
              </ReplicaDropZone>
            </SortableContext>
          </div>
        </div>

        <DragOverlay>
          {activeId ? (
            <DragOverlayContent row={activeRow} label={activePaletteLabel} />
          ) : null}
        </DragOverlay>
      </DndContext>

      <p className="help-text">
        Drag widgets from the palette into the indicator. Drag to reorder.
        Accessibility is used to get the selected text. Check that it is enabled
        in Privacy settings.
      </p>

      {/* Custom Shell Widgets */}
      <div className="custom-widgets-section">
        <div className="section-header">
          <h3>Custom script widgets</h3>
          <button className="btn-small" onClick={handleNewWidget}>
            + Add
          </button>
        </div>

        {settings.shell_widgets.length === 0 && !editing && (
          <p className="help-text">
            No custom widgets yet. Add a shell script that returns text to display in the indicator.
          </p>
        )}

        {settings.shell_widgets.map((widget, i) => (
          <div
            key={widget.name}
            className={`shell-widget-item ${editingIndex === i ? "editing" : ""}`}
          >
            <div className="shell-widget-header">
              <span className="shell-widget-name">{widget.name}</span>
              <span className="shell-widget-meta">
                {widget.script_path ? "file" : "inline"} / {widget.interval_secs}s
              </span>
              <div className="shell-widget-actions">
                <button className="btn-icon" onClick={() => handleEditWidget(i)} title="Edit">
                  E
                </button>
                <button
                  className="btn-icon btn-danger"
                  onClick={() => handleDeleteWidget(i)}
                  title="Delete"
                >
                  X
                </button>
              </div>
            </div>
          </div>
        ))}

        {editing && (
          <div className="shell-widget-editor">
            <div className="form-group">
              <label>Name</label>
              <input
                type="text"
                value={editing.name}
                onChange={(e) => setEditing({ ...editing, name: e.target.value })}
                placeholder="e.g. cpu-usage"
              />
            </div>

            <div className="form-group">
              <label>Type</label>
              <div className="shell-mode-toggle">
                <button
                  className={`btn-toggle ${editing.mode === "inline" ? "active" : ""}`}
                  onClick={() => setEditing({ ...editing, mode: "inline" })}
                >
                  Inline script
                </button>
                <button
                  className={`btn-toggle ${editing.mode === "file" ? "active" : ""}`}
                  onClick={() => setEditing({ ...editing, mode: "file" })}
                >
                  Script file
                </button>
              </div>
            </div>

            {editing.mode === "inline" ? (
              <div className="form-group">
                <label>Script</label>
                <textarea
                  className="script-editor"
                  value={editing.script}
                  onChange={(e) => setEditing({ ...editing, script: e.target.value })}
                  placeholder={"echo $(date +%H:%M)"}
                  rows={4}
                  spellCheck={false}
                />
              </div>
            ) : (
              <div className="form-group">
                <label>Script path</label>
                <input
                  type="text"
                  value={editing.script_path}
                  onChange={(e) => setEditing({ ...editing, script_path: e.target.value })}
                  placeholder="~/scripts/my-widget.sh"
                />
              </div>
            )}

            <div className="form-group">
              <label>Refresh interval (seconds)</label>
              <input
                type="number"
                value={editing.interval_secs}
                onChange={(e) =>
                  setEditing({ ...editing, interval_secs: Math.max(1, parseInt(e.target.value) || 10) })
                }
                min={1}
                style={{ maxWidth: 100 }}
              />
            </div>

            <div className="shell-widget-editor-actions">
              <button className="btn-primary" onClick={handleSaveWidget} disabled={!editing.name.trim()}>
                Save
              </button>
              <button className="btn-secondary" onClick={handleCancelEdit}>
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="electron-apps-section">
        <h3>Enable selection observing in Electron apps</h3>
        <AppList
          items={settings.electron_apps}
          onAdd={handleAddElectronApp}
          onAddManual={handleAddManualElectronApp}
          onRemove={handleRemoveElectronApp}
        />
        <p className="help-text">
          Observing selection in Electron apps requires more performance.
        </p>
        <p className="help-text warning">
          Removing app from the list requires a re-login.
        </p>
      </div>
    </div>
  );
}
