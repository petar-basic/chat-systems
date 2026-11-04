export default function SavedEverywhere({ failed }: { failed: boolean }) {
  return (
    <>
      <p className="text-sm text-muted">
        These settings are the same in every workspace and on every device.
      </p>
      {failed && (
        <p role="alert" className="text-sm text-danger">
          The setting could not be saved. Try again.
        </p>
      )}
    </>
  );
}
