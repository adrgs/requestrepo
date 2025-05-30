import React from "react";

interface RequestCardProps {
  active: boolean;
  visited: boolean;
  title: string;
  time: string;
  new?: boolean;
  method: string;
  country?: string;
  detail: string;
  id: string;
  key: string;
  clickRequestAction: (action: string, id: string) => void;
  sessionId?: string;
}

export const RequestCard: React.FC<RequestCardProps> = ({
  active,
  visited,
  title,
  time,
  new: isNew,
  method,
  country,
  detail,
  id,
  clickRequestAction,
}) => {
  const handleClick = (): void => {
    clickRequestAction("select", id);
  };

  const handleDelete = (e: React.MouseEvent): void => {
    e.stopPropagation();
    clickRequestAction("delete", id);
  };

  let annotation: React.ReactNode = null;
  if (isNew === true) {
    annotation = <span className="count delete">NEW!</span>;
  }

  return (
    <div
      className={
        "card request summary " +
        (active ? "active " : "") +
        (visited ? "visited " : "")
      }
      onClick={handleClick}
    >
      {annotation}
      <span className={"count " + method.toLowerCase()}>{method}</span>
      <span className="title">{title}</span>
      <span className="count bigx" onClick={handleDelete}>
        X
      </span>
      <span className="detail">
        {country && (
          <span
            style={{ marginRight: "5px" }}
            className={"fi fi-" + country.toLowerCase()}
          ></span>
        )}
        {detail}
        <span style={{ float: "right" }}>{time}</span>
      </span>
    </div>
  );
};
