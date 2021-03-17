<?php
include_once("includes/controller.php");
if(!$session->isAdmin()){
    header("Location: ".$configs->homePage());
    exit;
}
?>
<!-- Top Navbar -->
<nav class="navbar navbar-static-top" role="navigation">
    <a class="close-sidebar btn btn-main" href="#"><i class="oi oi-menu"></i> </a>
    <form class="search-form hidden-xs">
        <input class="searchbox" id="searchbox" type="text" placeholder="Search">
        <span class="searchbutton"><i class="oi oi-magnifying-glass"></i></span>
    </form>
    <a href="#" class="reveal-rightsidebar btn btn-main"><i class="oi oi-chevron-left"></i> </a>
    <a href="logout.php?path=admin" class="navbar-top-icons btn btn-main" data-original-title="Logout" data-toggle="tooltip" data-placement="bottom"><i class="oi oi-power-standby"></i> </a>
    <!-- Settings -->
    <div class="btn-group pull-right">
        <button type="button" class="btn btn-main navbar-top-icons dropdown-toggle" data-toggle="dropdown" aria-expanded="false">
            <i class="oi oi-cog"></i>
        </button>
        <ul class="dropdown-menu animated fadeIn" role="menu">
            <li class="dropdown-header">Settings</li>
            <li><a href="configurations.php"><i class="oi oi-cog"></i>General Settings</a></li>
            <li><a href="registration.php"><i class="oi oi-envelope-closed"></i>Registration Settings</a></li>
            <li><a href="session-settings.php"><i class="oi oi-globe"></i>Session Settings</a></li>
            <li><a href="security.php"><i class="oi oi-lock-locked"></i>Security Settings</a></li>
            <li><a href="user-settings.php"><i class="oi oi-person"></i>User Settings</a></li>
        </ul>
    </div>
    <!-- User -->
    <div class="btn-group pull-right">
        <button type="button" class="btn btn-main navbar-top-icons dropdown-toggle" data-toggle="dropdown" aria-expanded="false">
            <i class="oi oi-person"></i>
        </button>
        <ul class="dropdown-menu profile-dropdown animated fadeIn" role="menu">
            <li class="dropdown-header"><?php echo $session->username; ?></li>
            <li><a href="adminuseredit.php?usertoedit=<?php echo $session->username; ?>"><i class="oi oi-person"></i>Profile</a></li>
            <li><a href="logout.php?path=admin"><i class="oi oi-power-standby"></i>Log Out</a></li>
        </ul>
    </div>
    <div id="toggle">
        <a id="btn-fullscreen" class="navbar-top-icons btn btn-main hidden-xs" data-original-title="Fullscreen" data-toggle="tooltip" data-placement="bottom" href="#"><i id="toggle" class="oi oi-fullscreen-enter"></i> </a>
    </div>      
</nav> 
<!-- END Top Navbar -->

