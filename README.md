![Pillar logo](media/logo.png "Pillar :)")

> if you're burned out from programming,
> then maybe you should look at it from a different...<br>
> ***a***<br>
> ***n***<br>
> ***g***<br>
> ***l***<br>
> ***e***<br>
> ***?***

Pillar is an unusual column-based language.
In other languages you might see something like this:

```c
int main() {
  return 5+4 - 2*3;
}
```

Same program in pillar language look like this:

```plr
F   }
N    
  R  
m E  
a T  
i    
n 5  
(    
) +  
     
{ 2  
  *  
  5  
  ;  

```

# Installation

## Linux && macOS

On Unix-like systems, you can install the Pillar compiler with just:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/roazanas/pillar/releases/download/v0.2.1/pillar-installer.sh | sh
```

## Windows

> [!NOTE]
> On Windows you need to install linker (GCC or other)
> from external sources (if you don't already have one)

Install GCC using one of these methods:

### Option 1 (Recommended - Scoop)

```ps1
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
irm get.scoop.sh | iex
scoop install mingw
```

### Option 2 (Chocolatey)

```ps1
Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
choco install mingw
```

### Option 3 (winget)

```ps1
winget install MSYS2.MSYS2
```

Then, in MSYS2, run:

```bash
pacman -S mingw-w64-ucrt-x86_64-gcc
```

---

Once GCC or another linker is installed, run following to install the Pillar compiler:

```ps1
powershell -ExecutionPolicy Bypass -c "irm https://github.com/roazanas/pillar/releases/download/v0.2.1/pillar-installer.ps1 | iex"
```

> (it will actually install without GCC, but you need linker to compile Pillar program)
