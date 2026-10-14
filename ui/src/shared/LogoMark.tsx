/**
 * The RL Trainer "Ramp leg" symbol: an R whose bowl holds the ball and whose leg is a quarter-pipe.
 * This is the small-size cut (bigger ball) from the brand kit, cropped tight to the artwork.
 * It paints with currentColor, so the colour comes from CSS.
 */
interface LogoMarkProps {
  className?: string;
}

export function LogoMark({ className }: LogoMarkProps) {
  return (
    <svg className={className} viewBox="38 32 180 192" fill="currentColor" aria-hidden="true" focusable="false">
      <path
        fillRule="evenodd"
        d="M38 32H134a54 54 0 0 1 0 108H74V224H38ZM105 86a29 29 0 1 0 58 0a29 29 0 1 0 -58 0Z"
      />
      <path d="M98 132V140A84 84 0 0 0 182 224H218V188H182A48 48 0 0 1 134 140V132Z" />
    </svg>
  );
}
