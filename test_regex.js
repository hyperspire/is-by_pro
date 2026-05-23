const regex = /:\[\[ :project-invite: (\d+) \]\]:/g;
const str = "I have invited you to collaborate on my project: **Test**. \n\n:[[ :project-invite: 123 ]]:";
let m;
while ((m = regex.exec(str)) !== null) {
    console.log(m);
}
