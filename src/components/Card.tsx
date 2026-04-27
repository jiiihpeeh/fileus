import { JSX } from "solid-js";

interface CardProps {
  title: string;
  children: JSX.Element;
}

export function Card(props: CardProps) {
  return (
    <div class="card">
      <h2>{props.title}</h2>
      {props.children}
    </div>
  );
}

export default Card;