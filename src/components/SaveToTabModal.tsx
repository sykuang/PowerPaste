import { useState } from "react";

interface SaveToTabModalProps {
  categories: string[];
  onSave: (category: string) => void;
  onCancel: () => void;
}

export function SaveToTabModal(props: SaveToTabModalProps) {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(
    props.categories[0] ?? null
  );
  const [isCreatingNew, setIsCreatingNew] = useState(props.categories.length === 0);
  const [newCategoryName, setNewCategoryName] = useState("");

  const handleSave = () => {
    if (isCreatingNew) {
      const name = newCategoryName.trim();
      if (name) {
        props.onSave(name);
      }
    } else if (selectedCategory) {
      props.onSave(selectedCategory);
    }
  };

  const canSave = isCreatingNew ? newCategoryName.trim().length > 0 : selectedCategory !== null;

  return (
    <div className="modalBackdrop" onClick={props.onCancel}>
      <div className="modal saveToTabModal" onClick={(e) => e.stopPropagation()}>
        <div className="modalHeader">
          <div className="modalTitle">Save to Tab</div>
          <button
            type="button"
            className="btn"
            onClick={props.onCancel}
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        <div className="section">
          {props.categories.length > 0 && !isCreatingNew && (
            <>
              <label className="label">Choose a tab:</label>
              <div className="categoryList">
                {props.categories.map((cat) => (
                  <button
                    key={cat}
                    type="button"
                    className={`categoryOption${selectedCategory === cat ? " isSelected" : ""}`}
                    onClick={() => setSelectedCategory(cat)}
                  >
                    {cat}
                  </button>
                ))}
              </div>
              <button
                type="button"
                className="btn createNewBtn"
                onClick={() => {
                  setIsCreatingNew(true);
                  setSelectedCategory(null);
                }}
              >
                + Create new tab
              </button>
            </>
          )}

          {(isCreatingNew || props.categories.length === 0) && (
            <>
              <label className="label">New tab name:</label>
              <input
                type="text"
                className="input"
                value={newCategoryName}
                onChange={(e) => setNewCategoryName(e.target.value)}
                placeholder="e.g., Work Links, API Keys..."
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === "Enter" && canSave) {
                    handleSave();
                  }
                  if (e.key === "Escape") {
                    props.onCancel();
                  }
                }}
              />
              {props.categories.length > 0 && (
                <button
                  type="button"
                  className="btn backBtn"
                  onClick={() => {
                    setIsCreatingNew(false);
                    setSelectedCategory(props.categories[0] ?? null);
                  }}
                >
                  ← Back to existing tabs
                </button>
              )}
            </>
          )}
        </div>

        <div className="modalFooter">
          <button type="button" className="btn" onClick={props.onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="btnPrimary"
            onClick={handleSave}
            disabled={!canSave}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
