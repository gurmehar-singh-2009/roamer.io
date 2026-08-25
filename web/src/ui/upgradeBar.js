export class UpgradeBar {
  constructor(game) {
    this.isActive = false;
    this.game = game;

    this.element = document.createElement("div");
    this.element.style = `
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      width: 100%;
      height: 67px;
      position: absolute;
      top: 20px;
      z-index: 1000;
    `;
    document.body.appendChild(this.element);
  }

  toggleVisibility(isActive) {
    this.isActive = isActive;
    this.element.style.display = this.isActive ? "flex" : "none";
  }

  setWeapons(weapons) {
    for (let i = 0; i < weapons.length; i++) {
      let weapon = weapons[i];

      let rotation = (i * Math.PI * 2) / weapons.length;
      const weaponElement = document.createElement("canvas");
      weaponElement.width = 67;
      weaponElement.height = 67;
      weaponElement.style = `
        background-color: rgba(0, 0, 0, 0.2);
        border-radius: 4px;
        `;

      let hoverElement = document.createElement("div");
      let [name, description] = weapon.hoverDetails;
      hoverElement.innerHTML = `
        <h1 style="color: rgba(255, 255, 255, 0.8)">${name}</h1>
        <p style="color: rgba(255, 255, 255, 0.8)">${description}</p>
        `;
      hoverElement.style.display = "none";
      weaponElement.addEventListener("mouseenter", () => {
        hoverElement.style = `
          display: block;
          position: absolute;
          z-index: 10001;
          top: 100px;
          left: ${weaponElement.offsetLeft}px;
          background-color: rgba(0, 0, 0, 0.2);
          padding: 10px;
        `;
      });
      weaponElement.addEventListener("mouseleave", () => {
        hoverElement.style.display = "none";
      });
      document.body.appendChild(hoverElement);

      const ctx = weaponElement.getContext("2d");

      let img = new Image();
      img.src = weapon.texture;
      let rotateAnim = () => {
        ctx.clearRect(0, 0, weaponElement.width, weaponElement.height);
        ctx.save();
        ctx.translate(33.5, 33.5);
        ctx.rotate(rotation);
        ctx.drawImage(img, -33.5, -33.5, 67, 67);
        ctx.restore();

        rotation += 0.0067;
        
        requestAnimationFrame(rotateAnim);
      };
      img.onload = () => {
        rotateAnim();
      };

      weaponElement.addEventListener("click", () => {
        this.toggleVisibility(false);

        (async () => {
          console.error(weapon.id);
          await this.game.chooseWeapon(weapon.id);
        })();
      });

      this.element.appendChild(weaponElement);
    }
  }
}
