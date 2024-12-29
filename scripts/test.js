async function main() {
  // let now = Date.now();
  // const res = await fetch(`http://192.168.1.149/time?${now}`);
  const res = await fetch(`http://192.168.1.149/toggle/manual_mode`);
  // const res = await fetch(`http://192.168.1.149/get_info`);
  // const res = await fetch(
  //   `http://192.168.1.149/update_aspersor/toberas_afuera?duration=${1000000000}&init_time=${1500}`
  // );
  // const res = await fetch("http://espressif/");
  // const text = await res.text();
  const text = await res.json();
  console.log("text: ", text);
}

main()
  .then(console.log("done!"))
  .catch((e) => console.log("error: ", e));
