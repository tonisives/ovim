import { useState } from "react";

interface Props {
  items: string[];
  onAdd: () => void;
  onAddManual: (bundleId: string) => void;
  onRemove: (item: string) => void;
}

export function AppList({ items, onAdd, onAddManual, onRemove }: Props) {
  const [showManualInput, setShowManualInput] = useState(false);
  const [manualBundleId, setManualBundleId] = useState("");

  const handleManualSubmit = () => {
    const trimmed = manualBundleId.trim();
    if (trimmed) {
      onAddManual(trimmed);
      setManualBundleId("");
      setShowManualInput(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleManualSubmit();
    } else if (e.key === "Escape") {
      setManualBundleId("");
      setShowManualInput(false);
    }
  };

  return (
    <div className="app-list">
      <ul className="app-list-items">
        {items.map((item) => (
          <li key={item} className="app-list-item">
            <span className="app-bundle-id">{item}</span>
            <button
              className="remove-button"
              onClick={() => onRemove(item)}
              title="Remove"
            >
              {"\u2715"}
            </button>
          </li>
        ))}
        {items.length === 0 && (
          <li className="app-list-empty">No apps configured</li>
        )}
      </ul>
      {showManualInput ? (
        <div className="manual-bundle-input">
          <input
            type="text"
            value={manualBundleId}
            onChange={(e) => setManualBundleId(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="com.example.app"
            autoFocus
          />
          <button
            className="manual-submit-btn"
            onClick={handleManualSubmit}
            disabled={!manualBundleId.trim()}
          >
            Add
          </button>
          <button
            className="manual-cancel-btn"
            onClick={() => {
              setManualBundleId("");
              setShowManualInput(false);
            }}
          >
            Cancel
          </button>
        </div>
      ) : (
        <div className="add-buttons">
          <button className="add-button" onClick={onAdd}>
            + Browse Application
          </button>
          <button
            className="add-button add-manual"
            onClick={() => setShowManualInput(true)}
          >
            + Enter Bundle ID
          </button>
        </div>
      )}
    </div>
  );
}
