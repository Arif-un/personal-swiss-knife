/** Shared centered-overlay shell for the app's modal dialogs. Each caller
 *  provides its own panel as `children`; this only DRYs the backdrop. */
export function Modal({ children }: { children: React.ReactNode }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      {children}
    </div>
  );
}
