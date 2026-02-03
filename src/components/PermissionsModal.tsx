import { PermissionsStatus } from "../api";

interface PermissionsModalProps {
  checking: boolean;
  status: PermissionsStatus | null;
  onClose: () => void;
  onRecheck: () => Promise<void>;
  onOpenAccessibility: () => void;
  onOpenAutomation: () => void;
  closeOnBackdrop?: boolean;
}

export function PermissionsModal(props: PermissionsModalProps) {
  const status = props.status;
  const isMac = status?.platform === "macos";
  const canPaste = status?.can_paste ?? false;
  const closeOnBackdrop = props.closeOnBackdrop ?? true;

  return (
    <div 
      className={closeOnBackdrop ? "modalBackdrop" : "modalBackdrop modalBackdropStatic"}
      onClick={closeOnBackdrop ? props.onClose : undefined}
    >
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
            <div className="rowInline equalButtons">
              <button className="btn" onClick={props.onOpenAccessibility}>
                Open Accessibility
              </button>
              <button className="btn" onClick={props.onOpenAutomation}>
                Open Automation
              </button>
            </div>
            
            {status?.is_bundled === false ? (
              <>
                <div className="warningInline">
                  ⚠️ Running in <strong>development mode</strong>. You need to add the debug binary to permissions:
                </div>
                <div className="hint" style={{ fontFamily: "monospace", fontSize: 11, wordBreak: "break-all" }}>
                  {status.executable_path}
                </div>
                <ol className="hintList">
                  <li><strong>Accessibility:</strong> Click +, press Cmd+Shift+G, paste the path above, and enable it.</li>
                  <li><strong>Automation:</strong> The binary should appear after running — enable "System Events".</li>
                </ol>
              </>
            ) : (
              <>
                <div className="hint">
                  <strong>Important:</strong> Make sure to add <strong>PowerPaste.app</strong> to both lists.
                </div>
                <ol className="hintList">
                  <li><strong>Accessibility:</strong> Click +, navigate to Applications, select PowerPaste.app, and enable it.</li>
                  <li><strong>Automation:</strong> Find PowerPaste in the list and enable "System Events".</li>
                </ol>
              </>
            )}
            <div className="hint">
              After granting permissions, <strong>restart PowerPaste</strong> for changes to take effect.
            </div>
          </div>
        ) : null}

        <div className="modalFooter">
          <button className="btn" disabled={props.checking} onClick={() => void props.onRecheck()}>
            Re-check
          </button>
          {canPaste ? (
            <button className="btnPrimary" disabled={props.checking} onClick={props.onClose}>
              Done
            </button>
          ) : (
            <button 
              className="btnPrimary" 
              disabled={props.checking} 
              onClick={() => {
                props.onOpenAccessibility();
                // Also open Automation after a short delay
                setTimeout(() => props.onOpenAutomation(), 500);
              }}
            >
              Open System Settings
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
