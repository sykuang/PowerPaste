import { PermissionsStatus } from "../api";

interface PermissionsModalProps {
  checking: boolean;
  status: PermissionsStatus | null;
  onClose: () => void;
  onRecheck: () => Promise<void>;
  onOpenAccessibility: () => void;
  onOpenAutomation: () => void;
}

export function PermissionsModal(props: PermissionsModalProps) {
  const status = props.status;
  const isMac = status?.platform === "macos";
  const canPaste = status?.can_paste ?? false;

  return (
    <div className="modalBackdrop" onClick={props.onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modalHeader">
          <div className="modalTitle">Finish setup</div>
          <button className="btn" onClick={props.onClose}>
            Skip
          </button>
        </div>

        <div className="section">
          <div className="hint">
            PowerPaste can copy immediately. To “paste into other apps” on double-click, macOS requires additional
            permissions.
          </div>
        </div>

        <div className="section">
          <div className="label">Status</div>
          <div className="hint">
            {props.checking
              ? "Checking permissions…"
              : canPaste
                ? "All set — paste is enabled."
                : isMac
                  ? "Paste is not enabled yet."
                  : "Paste automation is currently only supported on macOS."}
          </div>

          {status ? (
            <div className="hint">
              Accessibility: {status.accessibility_ok ? "OK" : "Missing"} • Automation: {status.automation_ok ? "OK" : "Missing"}
            </div>
          ) : null}

          {status?.details ? <div className="error">{status.details}</div> : null}
        </div>

        {isMac ? (
          <div className="section">
            <div className="label">Grant permissions</div>
            <div className="rowInline">
              <button className="btn" onClick={props.onOpenAccessibility}>
                Open Accessibility
              </button>
              <button className="btn" onClick={props.onOpenAutomation}>
                Open Automation
              </button>
            </div>
            <div className="hint">
              Enable PowerPaste in Accessibility, and allow controlling System Events in Automation.
            </div>
          </div>
        ) : null}

        <div className="modalFooter">
          <button className="btn" disabled={props.checking} onClick={() => void props.onRecheck()}>
            Re-check
          </button>
          <button className="btnPrimary" disabled={props.checking || !canPaste} onClick={props.onClose}>
            Continue
          </button>
        </div>
      </div>
    </div>
  );
}
