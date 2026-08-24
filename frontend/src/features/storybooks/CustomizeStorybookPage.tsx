import { Navigate, useLocation, useOutletContext, useParams } from "react-router-dom";
import { EmptyState } from "../../components/ui";
import type { Workspace } from "../../types/domain";

export function CustomizeStorybookPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const { storybookId } = useParams();
  const location = useLocation();

  if (!storybookId) {
    return (
      <EmptyState
        title="没有找到来源绘本"
        copy="请回到绘本列表，选择一本普通绘本后再创作专属版本。"
      />
    );
  }

  const query = new URLSearchParams(location.search);
  query.set("sourceStorybookId", storybookId);

  return <Navigate to={`/app/${workspace.id}/storybooks/personalized/new?${query.toString()}`} replace />;
}
