import { useState } from "react";

interface SaveToPinboardModalProps {
  pinboards: string[];
  onSave: (pinboard: string) => void;
  onCancel: () => void;
}

export function SaveToPinboardModal(props: SaveToPinboardModalProps) {
  const [selectedPinboard, setSelectedPinboard] = useState<string | null>(
    props.pinboards[0] ?? null
  );
  const [isCreatingNew, setIsCreatingNew] = useState(props.pinboards.length === 0);
  const [newPinboardName, setNewPinboardName] = useState("");

  const handleSave = () => {
    if (isCreatingNew) {
      const name = newPinboardName.trim();
      if (name) {
        props.onSave(name);
      }
    } else if (selectedPinboard) {
      props.onSave(selectedPinboard);
    }
  };

  const canSave = isCreatingNew ? newPinboardName.trim().length > 0 : selectedPinboard !== null;

  return (
    <div className="modalBackdrop" onClick={props.onCancel}>
      <div className="modal saveToPinboardModal" onClick={(e) => e.stopPropagation()}>
        <div className="modalHeader">
          <div className="modalTitle">Save to Pinboard</div>
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
          {props.pinboards.length > 0 && !isCreatingNew && (
            <>
              <label className="label">Choose a pinboard:</label>
              <div className="pinboardList">
                {props.pinboards.map((pb) => (
                  <button
                    key={pb}
                    type="button"
                    className={`pinboardOption${selectedPinboard === pb ? " isSelected" : ""}`}
                    onClick={() => setSelectedPinboard(pb)}
                  >
                    {pb}
                  </button>
                ))}
              </div>
              <button
                type="button"
                className="btn createNewBtn"
                onClick={() => {
                  setIsCreatingNew(true);
                  setSelectedPinboard(null);
                }}
              >
                + Create new pinboard
              </button>
            </>
          )}

          {(isCreatingNew || props.pinboards.length === 0) && (
            <>
              <label className="label">New pinboard name:</label>
              <input
                type="text"
                className="input"
                value={newPinboardName}
                onChange={(e) => setNewPinboardName(e.target.value)}
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
              {props.pinboards.length > 0 && (
                <button
                  type="button"
                  className="btn backBtn"
                  onClick={() => {
                    setIsCreatingNew(false);
                    setSelectedPinboard(props.pinboards[0] ?? null);
                  }}
                >
                  ← Back to existing pinboards
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
